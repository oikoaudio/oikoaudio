//! Official ODDsound libMTS ABI, as specified by Master/libMTSMaster.cpp.
//! All dynamic loading and MTS calls run on the control thread, never process().
use std::{
    ffi::{CString, c_char, c_void},
    path::PathBuf,
    sync::Mutex,
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MasterStatus {
    Disabled,
    ActiveMaster,
    BlockedByOtherMaster,
    Unavailable,
    Error,
}
impl MasterStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Disabled => "Disabled",
            Self::ActiveMaster => "Master active",
            Self::BlockedByOtherMaster => "Another master is active",
            Self::Unavailable => "MTS library unavailable",
            Self::Error => "MTS error",
        }
    }
}
pub trait MasterApi {
    fn available(&self) -> bool;
    fn can_register(&self) -> bool;
    fn register(&mut self);
    fn deregister(&mut self);
    fn publish(&mut self, hz: &[f64; 128], name: &str, period: f64);
    fn clients(&self) -> usize;
    fn supports_recovery(&self) -> bool {
        false
    }
    fn reinitialize(&mut self) {}
}
static OWNERSHIP: Mutex<()> = Mutex::new(());
pub struct Master<A: MasterApi> {
    api: A,
    owned: bool,
    last: Option<([f64; 128], String, f64)>,
}
impl<A: MasterApi> Master<A> {
    pub fn new(api: A) -> Self {
        Self {
            api,
            owned: false,
            last: None,
        }
    }
    pub fn clients(&self) -> usize {
        if self.owned { self.api.clients() } else { 0 }
    }
    pub fn update(
        &mut self,
        enabled: bool,
        hz: &[f64; 128],
        name: &str,
        period: f64,
    ) -> MasterStatus {
        self.update_requested(enabled, hz, name, period, false)
    }
    pub fn recovery_available(&self) -> bool {
        !self.owned
            && self.api.available()
            && self.api.supports_recovery()
            && !self.api.can_register()
    }
    /// Only call in response to the user's explicit reset action after a suspected crash.
    pub fn recover_and_update(
        &mut self,
        enabled: bool,
        hz: &[f64; 128],
        name: &str,
        period: f64,
    ) -> MasterStatus {
        self.update_requested(enabled, hz, name, period, true)
    }
    fn update_requested(
        &mut self,
        enabled: bool,
        hz: &[f64; 128],
        name: &str,
        period: f64,
        recover: bool,
    ) -> MasterStatus {
        let _guard = OWNERSHIP.lock().unwrap_or_else(|e| e.into_inner());
        if !enabled {
            if self.owned {
                self.api.deregister();
                self.owned = false;
            }
            self.last = None;
            return MasterStatus::Disabled;
        }
        if !self.api.available() {
            return MasterStatus::Unavailable;
        }
        if hz.iter().any(|f| !f.is_finite() || *f <= 0.) {
            return MasterStatus::Error;
        }
        if recover && self.recovery_available() {
            self.api.reinitialize();
            self.last = None;
        }
        if !self.owned {
            if !self.api.can_register() {
                return MasterStatus::BlockedByOtherMaster;
            }
            self.api.register();
            self.owned = true;
            self.last = None;
        }
        if self
            .last
            .as_ref()
            .is_none_or(|(last, n, p)| last != hz || n != name || *p != period)
        {
            self.api.publish(hz, name, period);
            self.last = Some((*hz, name.to_owned(), period));
        }
        MasterStatus::ActiveMaster
    }
}
impl<A: MasterApi> Drop for Master<A> {
    fn drop(&mut self) {
        let _guard = OWNERSHIP.lock().unwrap_or_else(|e| e.into_inner());
        if self.owned {
            self.api.deregister()
        }
    }
}
type Void = unsafe extern "C" fn();
struct Functions {
    register: unsafe extern "C" fn(*mut c_void),
    deregister: Void,
    has_master: unsafe extern "C" fn() -> bool,
    has_ipc: Option<unsafe extern "C" fn() -> bool>,
    reinitialize: Option<Void>,
    set: unsafe extern "C" fn(*const f64),
    name: unsafe extern "C" fn(*const c_char),
    period: Option<unsafe extern "C" fn(f64)>,
    clear: Void,
    multi: unsafe extern "C" fn(bool, i8),
    map_size: Option<unsafe extern "C" fn(i8)>,
    map_start: Option<unsafe extern "C" fn(i8)>,
    ref_key: Option<unsafe extern "C" fn(i8)>,
    clients: unsafe extern "C" fn() -> i32,
}
pub struct Native {
    functions: Option<Functions>,
    _library: Option<libloading::Library>,
}
impl Default for Native {
    fn default() -> Self {
        Self::load()
    }
}
impl Native {
    pub fn load() -> Self {
        let path = std::env::var_os("INTON_MTS_LIBRARY")
            .map(PathBuf::from)
            .unwrap_or_else(default_library_path);
        unsafe {
            let Ok(lib) = libloading::Library::new(path) else {
                return Self {
                    functions: None,
                    _library: None,
                };
            };
            let load = || -> Result<Functions, libloading::Error> {
                Ok(Functions {
                    register: *lib.get(b"MTS_RegisterMaster\0")?,
                    deregister: *lib.get(b"MTS_DeregisterMaster\0")?,
                    has_master: *lib.get(b"MTS_HasMaster\0")?,
                    has_ipc: lib.get(b"MTS_HasIPC\0").ok().map(|f| *f),
                    reinitialize: lib.get(b"MTS_Reinitialize\0").ok().map(|f| *f),
                    set: *lib.get(b"MTS_SetNoteTunings\0")?,
                    name: *lib.get(b"MTS_SetScaleName\0")?,
                    period: lib.get(b"MTS_SetPeriodRatio\0").ok().map(|f| *f),
                    clear: *lib.get(b"MTS_ClearNoteFilter\0")?,
                    multi: *lib.get(b"MTS_SetMultiChannel\0")?,
                    map_size: lib.get(b"MTS_SetMapSize\0").ok().map(|f| *f),
                    map_start: lib.get(b"MTS_SetMapStartKey\0").ok().map(|f| *f),
                    ref_key: lib.get(b"MTS_SetRefKey\0").ok().map(|f| *f),
                    clients: *lib.get(b"MTS_GetNumClients\0")?,
                })
            };
            match load() {
                Ok(f) => Self {
                    functions: Some(f),
                    _library: Some(lib),
                },
                Err(_) => Self {
                    functions: None,
                    _library: None,
                },
            }
        }
    }
}
#[cfg(target_os = "linux")]
fn default_library_path() -> PathBuf {
    "/usr/local/lib/libMTS.so".into()
}
#[cfg(target_os = "macos")]
fn default_library_path() -> PathBuf {
    "/Library/Application Support/MTS-ESP/libMTS.dylib".into()
}
#[cfg(target_os = "windows")]
fn default_library_path() -> PathBuf {
    let program_files = if cfg!(target_pointer_width = "32") {
        std::env::var_os("ProgramFiles(x86)").or_else(|| std::env::var_os("ProgramFiles"))
    } else {
        std::env::var_os("ProgramFiles")
    };
    program_files
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files"))
        .join(r"Common Files\MTS-ESP\libMTS.dll")
}
impl MasterApi for Native {
    fn available(&self) -> bool {
        self.functions.is_some()
    }
    fn can_register(&self) -> bool {
        self.functions
            .as_ref()
            .is_some_and(|f| unsafe { !(f.has_master)() })
    }
    fn supports_recovery(&self) -> bool {
        self.functions.as_ref().is_some_and(|f| {
            f.reinitialize.is_some() && f.has_ipc.is_some_and(|ipc| unsafe { ipc() })
        })
    }
    fn reinitialize(&mut self) {
        if self.supports_recovery()
            && let Some(reset) = self.functions.as_ref().and_then(|f| f.reinitialize)
        {
            unsafe { reset() }
        }
    }
    fn register(&mut self) {
        if let Some(f) = &self.functions {
            unsafe {
                (f.register)(std::ptr::null_mut());
                (f.clear)();
                for channel in 0..16 {
                    (f.multi)(false, channel)
                }
                for reset in [f.map_size, f.map_start, f.ref_key].into_iter().flatten() {
                    reset(-1)
                }
            }
        }
    }
    fn deregister(&mut self) {
        if let Some(f) = &self.functions {
            unsafe { (f.deregister)() }
        }
    }
    fn publish(&mut self, hz: &[f64; 128], name: &str, period: f64) {
        if let Some(f) = &self.functions {
            let name = CString::new(name.replace('\0', " ")).unwrap_or_default();
            unsafe {
                (f.set)(hz.as_ptr());
                (f.name)(name.as_ptr());
                if let Some(set_period) = f.period {
                    set_period(period)
                }
            }
        }
    }
    fn clients(&self) -> usize {
        self.functions
            .as_ref()
            .map_or(0, |f| unsafe { (f.clients)().max(0) as usize })
    }
}
