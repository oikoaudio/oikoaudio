//! Headless rendering of the actual egui editor for visual regression inspection.
use egui::{Color32, TextureId};
use std::{collections::HashMap, sync::Arc};
fn main() {
    let args: Vec<_> = std::env::args().collect();
    // Scene flags are saved to the real preferences file; set XDG_CONFIG_HOME to a
    // scratch directory to keep your own settings.
    let mut prefs = inton::library::Preferences::load_from(&inton::library::preferences_path())
        .unwrap_or_default();
    prefs.view.listening_ideas = args.iter().any(|a| a == "ideas" || a == "guide");
    prefs.view.dark = !args.iter().any(|a| a == "light");
    prefs.view.browser_open = !args.iter().any(|a| a == "closed");
    prefs.save_to(&inton::library::preferences_path()).unwrap();
    let width = prefs.view.width() as usize;
    let height = prefs.view.height() as usize;
    let shared = Arc::new(inton_core::runtime::Shared::new());
    if args.iter().any(|a| a == "connected") {
        *shared.status.lock().unwrap() = (inton_core::mts::MasterStatus::ActiveMaster, 13);
    }
    if args.iter().any(|a| a == "recovery" || a == "blocked") {
        *shared.status.lock().unwrap() = (inton_core::mts::MasterStatus::BlockedByOtherMaster, 0);
        shared
            .recovery_available
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }
    if args.iter().any(|a| a == "soft-active") {
        shared
            .assign(
                0,
                inton::library::Library::factory()
                    .load("factory:detune-soft-circuit")
                    .unwrap(),
            )
            .unwrap();
    }
    if args.iter().any(|a| a == "dense") {
        let mut project = inton_core::state::Project::default();
        for i in 0..12 {
            let mut preset = inton_core::tuning::twelve_edo();
            preset.display_name = format!(
                "A very long scale name with Unicode ♯ and ratios — variation {}",
                i + 1
            );
            project.assign(i, preset).unwrap();
        }
        project.parameters.position = 11;
        shared.restore(project).unwrap();
    }
    if let Some(pair) = args.windows(2).find(|pair| pair[0] == "scl") {
        let path = std::path::Path::new(&pair[1]);
        let mut preset = inton_core::tuning::Preset::new(
            path.file_stem().unwrap().to_string_lossy().into_owned(),
            std::fs::read_to_string(path).unwrap(),
            std::fs::read_to_string(path.with_extension("kbm")).ok(),
        );
        preset.description = preset
            .scl_text
            .lines()
            .find(|line| !line.trim_start().starts_with('!'))
            .unwrap_or_default()
            .into();
        shared.assign(0, preset).unwrap();
    }
    if args.iter().any(|a| a == "wide") {
        shared
            .assign(
                0,
                inton::library::Library::factory()
                    .load("factory:17ed4")
                    .unwrap(),
            )
            .unwrap();
    }
    if args.iter().any(|a| a == "gaps") {
        let lib = inton::library::Library::factory();
        shared.append(lib.load("factory:19edo").unwrap()).unwrap();
        shared.append(lib.load("factory:22edo").unwrap()).unwrap();
        shared.clear(1).unwrap();
    }
    if args.iter().any(|a| a == "unassigned") {
        let mut parameters = shared.parameters.read().unwrap();
        parameters.position = 31;
        shared.parameters.write(parameters);
        shared.publication(0.);
    }
    let mut editor = inton::editor::Editor::new(shared, inton::plugin::HostRef::disconnected());
    if args.iter().any(|a| a == "reference") {
        editor.show_reference_ring();
    }
    if args.iter().any(|a| a == "project") {
        editor.show_project();
    }
    let ctx = egui::Context::default();
    let mut textures: HashMap<TextureId, egui::ColorImage> = HashMap::new();
    let mut shapes = vec![];
    for frame in 0..24 {
        if frame == 3 && std::env::args().any(|a| a == "selected") {
            editor.select("factory:22edo".into());
        }
        if frame == 3 && args.iter().any(|a| a == "soft") {
            editor.select("factory:detune-soft-circuit".into());
        }
        if frame == 4 && args.iter().any(|a| a == "edit") {
            editor.edit_selected().unwrap();
        }
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(width as f32, height as f32),
            )),
            events: if args.iter().any(|a| a == "hover-wheel") {
                vec![egui::Event::PointerMoved(egui::pos2(110., 122.))]
            } else if args.iter().any(|a| a == "guide" || a == "recovery")
                && (frame == 5 || frame == 6)
            {
                let pos = if args.iter().any(|a| a == "recovery") {
                    egui::pos2(if width == 570 { 640. } else { 380. }, 474.)
                } else {
                    egui::pos2(if width == 570 { 440. } else { 200. }, 263.)
                };
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: frame == 5,
                        modifiers: egui::Modifiers::NONE,
                    },
                ]
            } else {
                vec![]
            },
            time: Some(frame as f64 / 30.),
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| editor.draw(ui));
        for (id, deltas) in out.textures_delta.set.clone() {
            for delta in deltas {
                let egui::ImageData::Color(image) = delta.image;
                if let Some([x, y]) = delta.pos {
                    let target = textures.get_mut(&id).unwrap();
                    for j in 0..image.size[1] {
                        for i in 0..image.size[0] {
                            target.pixels[(j + y) * target.size[0] + i + x] =
                                image.pixels[j * image.size[0] + i];
                        }
                    }
                } else {
                    textures.insert(id, (*image).clone());
                }
            }
        }
        out.textures_delta.clear();
        shapes = out.shapes;
        std::thread::sleep(std::time::Duration::from_millis(15));
    }
    let mut canvas = vec![Color32::from_rgb(24, 29, 29); width * height];
    for prim in ctx.tessellate(shapes, 1.) {
        if let egui::epaint::Primitive::Mesh(m) = prim.primitive {
            let tex = textures.get(&m.texture_id).unwrap();
            for tri in m.indices.as_chunks::<3>().0 {
                let v = tri
                    .iter()
                    .map(|i| m.vertices[*i as usize])
                    .collect::<Vec<_>>();
                let minx = v
                    .iter()
                    .map(|v| v.pos.x)
                    .fold(f32::INFINITY, f32::min)
                    .floor()
                    .max(prim.clip_rect.min.x)
                    .max(0.) as usize;
                let maxx = v
                    .iter()
                    .map(|v| v.pos.x)
                    .fold(0., f32::max)
                    .ceil()
                    .min(prim.clip_rect.max.x)
                    .min(width as f32) as usize;
                let miny = v
                    .iter()
                    .map(|v| v.pos.y)
                    .fold(f32::INFINITY, f32::min)
                    .floor()
                    .max(prim.clip_rect.min.y)
                    .max(0.) as usize;
                let maxy = v
                    .iter()
                    .map(|v| v.pos.y)
                    .fold(0., f32::max)
                    .ceil()
                    .min(prim.clip_rect.max.y)
                    .min(height as f32) as usize;
                let cross = |a: egui::Vec2, b: egui::Vec2| a.x * b.y - a.y * b.x;
                let denom = cross(v[1].pos - v[0].pos, v[2].pos - v[0].pos);
                if denom.abs() < 1e-8 {
                    continue;
                }
                for y in miny..maxy {
                    for x in minx..maxx {
                        let p = egui::pos2(x as f32 + 0.5, y as f32 + 0.5);
                        let b = cross(p - v[0].pos, v[2].pos - v[0].pos) / denom;
                        let c = cross(v[1].pos - v[0].pos, p - v[0].pos) / denom;
                        let a = 1. - b - c;
                        if a < 0. || b < 0. || c < 0. {
                            continue;
                        }
                        let w = [a, b, c];
                        let uv =
                            v[0].uv.to_vec2() * a + v[1].uv.to_vec2() * b + v[2].uv.to_vec2() * c;
                        let tx = (uv.x * tex.size[0] as f32)
                            .floor()
                            .clamp(0., (tex.size[0] - 1) as f32)
                            as usize;
                        let ty = (uv.y * tex.size[1] as f32)
                            .floor()
                            .clamp(0., (tex.size[1] - 1) as f32)
                            as usize;
                        let t = tex.pixels[ty * tex.size[0] + tx].to_array();
                        let mut color = [0.; 4];
                        for k in 0..4 {
                            color[k] = v
                                .iter()
                                .zip(w)
                                .map(|(v, w)| v.color.to_array()[k] as f32 * w)
                                .sum::<f32>()
                                * t[k] as f32
                                / 255.;
                        }
                        let dest = canvas[y * width + x].to_array();
                        let mut out = [255u8; 4];
                        for k in 0..3 {
                            out[k] = (color[k] + dest[k] as f32 * (1. - color[3] / 255.))
                                .clamp(0., 255.) as u8;
                        }
                        canvas[y * width + x] =
                            Color32::from_rgba_premultiplied(out[0], out[1], out[2], 255);
                    }
                }
            }
        }
    }
    let path = std::env::args().nth(1).unwrap_or("ui-snapshot.png".into());
    let file = std::fs::File::create(&path).unwrap();
    let mut encoder = png::Encoder::new(file, width as u32, height as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    let pixels: Vec<u8> = canvas.iter().flat_map(|c| c.to_array()).collect();
    writer.write_image_data(&pixels).unwrap();
    println!("{path}");
}
