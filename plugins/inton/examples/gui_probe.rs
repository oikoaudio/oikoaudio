//! Embedded CLAP editor lifecycle smoke test against the built shared object.

#[cfg(target_os = "linux")]
mod linux {
    use clap_sys::{
        entry::*,
        ext::{gui::*, params::*},
        factory::plugin_factory::*,
        host::*,
        version::*,
    };
    use std::{
        ffi::{CStr, c_char, c_void},
        ptr,
        sync::atomic::{AtomicBool, AtomicU64, Ordering::SeqCst},
        time::{Duration, Instant},
    };
    use x11rb::wrapper::ConnectionExt as _;
    use x11rb::{COPY_DEPTH_FROM_PARENT, connection::Connection, protocol::xproto::*};
    static RESIZE: AtomicU64 = AtomicU64::new(0);
    static ACCEPT_RESIZE: AtomicBool = AtomicBool::new(true);
    unsafe extern "C" fn resize(_: *const clap_host, w: u32, h: u32) -> bool {
        if ACCEPT_RESIZE.load(SeqCst) {
            RESIZE.store(((w as u64) << 32) | h as u64, SeqCst);
            true
        } else {
            false
        }
    }
    static GUI_HOST: clap_host_gui = clap_host_gui {
        resize_hints_changed: None,
        request_resize: Some(resize),
        request_show: None,
        request_hide: None,
        closed: None,
    };
    static CALLBACK: AtomicBool = AtomicBool::new(false);
    static FLUSH: AtomicBool = AtomicBool::new(false);
    unsafe extern "C" fn request(_: *const clap_host) {
        CALLBACK.store(true, SeqCst)
    }
    unsafe extern "C" fn request_flush(_: *const clap_host) {
        FLUSH.store(true, SeqCst)
    }
    static PARAMS: clap_host_params = clap_host_params {
        rescan: None,
        clear: None,
        request_flush: Some(request_flush),
    };
    unsafe extern "C" fn ext(_: *const clap_host, id: *const c_char) -> *const c_void {
        if CStr::from_ptr(id) == CLAP_EXT_PARAMS {
            (&PARAMS as *const clap_host_params).cast()
        } else if CStr::from_ptr(id) == CLAP_EXT_GUI {
            (&GUI_HOST as *const clap_host_gui).cast()
        } else {
            ptr::null()
        }
    }
    struct PluginGuard(*const clap_sys::plugin::clap_plugin);
    impl Drop for PluginGuard {
        fn drop(&mut self) {
            unsafe {
                if !self.0.is_null() {
                    (*self.0).destroy.unwrap()(self.0)
                }
            }
        }
    }
    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        unsafe {
            let path = std::env::args().nth(1).unwrap_or_else(|| {
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../target/bundled/Oiko Inton.clap")
                    .to_string_lossy()
                    .into_owned()
            });
            let lib = libloading::Library::new(path)?;
            let entry: libloading::Symbol<*const clap_plugin_entry> = lib.get(b"clap_entry\0")?;
            let entry = &**entry;
            assert!(entry.init.unwrap()(c"test".as_ptr()));
            let factory = &*(entry.get_factory.unwrap()(CLAP_PLUGIN_FACTORY_ID.as_ptr())
                as *const clap_plugin_factory);
            let host = clap_host {
                clap_version: CLAP_VERSION,
                host_data: ptr::null_mut(),
                name: c"Inton GUI test".as_ptr(),
                vendor: c"Oiko".as_ptr(),
                url: c"".as_ptr(),
                version: c"1".as_ptr(),
                get_extension: Some(ext),
                request_restart: None,
                request_process: None,
                request_callback: Some(request),
            };
            let plugin =
                factory.create_plugin.unwrap()(factory, &host, c"audio.oiko.inton".as_ptr());
            assert!(!plugin.is_null());
            let guard = PluginGuard(plugin);
            assert!((*plugin).init.unwrap()(plugin));
            let gui = &*((*plugin).get_extension.unwrap()(plugin, CLAP_EXT_GUI.as_ptr())
                as *const clap_plugin_gui);
            let params = &*((*plugin).get_extension.unwrap()(plugin, CLAP_EXT_PARAMS.as_ptr())
                as *const clap_plugin_params);
            let (conn, screen_num) = x11rb::connect(None)?;
            let screen = &conn.setup().roots[screen_num];
            for scale in [1.0, 1.25, 1.5, 2.0] {
                let parent = conn.generate_id()?;
                assert!(gui.create.unwrap()(
                    plugin,
                    CLAP_WINDOW_API_X11.as_ptr(),
                    false
                ));
                assert!(gui.set_scale.unwrap()(plugin, scale));
                let (mut w, mut h) = (0, 0);
                assert!(gui.get_size.unwrap()(plugin, &mut w, &mut h));
                assert!(w > 0 && h > 0);
                let (width, height) = (u16::try_from(w)?, u16::try_from(h)?);
                conn.create_window(
                    COPY_DEPTH_FROM_PARENT,
                    parent,
                    screen.root,
                    0,
                    0,
                    width,
                    height,
                    0,
                    WindowClass::INPUT_OUTPUT,
                    0,
                    &CreateWindowAux::new()
                        .background_pixel(screen.black_pixel)
                        .override_redirect(1),
                )?
                .check()?;
                conn.change_property8(
                    PropMode::REPLACE,
                    parent,
                    AtomEnum::WM_NAME,
                    AtomEnum::STRING,
                    b"Oiko Inton - editor test",
                )?
                .check()?;
                conn.map_window(parent)?.check()?;
                conn.flush()?;
                let window = clap_window {
                    api: CLAP_WINDOW_API_X11.as_ptr(),
                    specific: clap_window_handle { x11: parent as u64 },
                };
                assert!(gui.set_parent.unwrap()(plugin, &window));
                assert!(gui.show.unwrap()(plugin));
                let started = Instant::now();
                while started.elapsed() < Duration::from_millis(1000) {
                    if CALLBACK.swap(false, SeqCst) {
                        (*plugin).on_main_thread.unwrap()(plugin)
                    }
                    if FLUSH.swap(false, SeqCst) {
                        params.flush.unwrap()(plugin, ptr::null(), ptr::null());
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                assert!(!conn.query_tree(parent)?.reply()?.children.is_empty());
                let child = conn.query_tree(parent)?.reply()?.children[0];
                let pump = || -> Result<(), Box<dyn std::error::Error>> {
                    let started = Instant::now();
                    while started.elapsed() < Duration::from_millis(400) {
                        if CALLBACK.swap(false, SeqCst) {
                            (*plugin).on_main_thread.unwrap()(plugin);
                        }
                        if FLUSH.swap(false, SeqCst) {
                            params.flush.unwrap()(plugin, ptr::null(), ptr::null());
                        }
                        let resize = RESIZE.swap(0, SeqCst);
                        if resize != 0 {
                            conn.configure_window(
                                parent,
                                &ConfigureWindowAux::new()
                                    .width((resize >> 32) as u32)
                                    .height(resize as u32),
                            )?
                            .check()?;
                            conn.flush()?;
                        }
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Ok(())
                };
                pump()?;
                let geometry = conn.get_geometry(child)?.reply()?;
                assert_eq!((geometry.width, geometry.height), (width, height));
                let snapshot = conn
                    .get_image(ImageFormat::Z_PIXMAP, child, 0, 0, width, height, u32::MAX)?
                    .reply()?;
                assert!(
                    snapshot
                        .data
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .any(|p| p[0] != p[1]),
                    "Editor did not render colored pixels"
                );
                let path = format!("/tmp/inton-native-{}.png", (scale * 100.) as u32);
                let file = std::fs::File::create(path)?;
                let mut encoder = png::Encoder::new(file, width as u32, height as u32);
                encoder.set_color(png::ColorType::Rgb);
                encoder.set_depth(png::BitDepth::Eight);
                let rgb: Vec<u8> = snapshot
                    .data
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .flat_map(|pixel| [pixel[2], pixel[1], pixel[0]])
                    .collect();
                encoder.write_header()?.write_image_data(&rgb)?;
                assert!(gui.hide.unwrap()(plugin));
                assert!(gui.show.unwrap()(plugin));
                pump()?;
                assert_eq!(conn.get_geometry(child)?.reply()?.width, width);
                gui.destroy.unwrap()(plugin);
                assert!(conn.query_tree(parent)?.reply()?.children.is_empty());
                println!(
                    "Embedded editor lifecycle and rendering passed at {}%",
                    (scale * 100.) as u32
                );
                conn.destroy_window(parent)?.check()?;
                conn.flush()?;
            }
            drop(guard);
            entry.deinit.unwrap()();
            println!(
                "Embedded editor: create/parent/show/hide/reopen/destroy and recreation passed at 100%, 125%, 150% and 200% scale. Browser interaction is covered by the headless UI tests."
            );
            Ok(())
        }
    }
}

#[cfg(target_os = "linux")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    linux::run()
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("gui_probe exercises X11/XWayland and only runs on Linux.");
}
