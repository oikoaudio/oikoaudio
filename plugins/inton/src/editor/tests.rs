use super::*;
use inton_core::tuning::Preset;

mod browsing;
mod connection;
mod listening;
mod scale_edit;
mod scale_set;

fn settle(e: &mut Editor) {
    for _ in 0..200 {
        e.poll();
        if e.job.is_none() && e.queue.is_empty() {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("Library command did not finish");
}

fn frame(e: &mut Editor, ctx: &egui::Context, time: &mut f64, events: Vec<egui::Event>) {
    frame_at_size(e, ctx, time, events, vec2(570., 500.))
}

fn frame_at_size(
    e: &mut Editor,
    ctx: &egui::Context,
    time: &mut f64,
    events: Vec<egui::Event>,
    size: egui::Vec2,
) {
    *time += 0.02;
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            time: Some(*time),
            events,
            ..Default::default()
        },
        |ui| e.draw(ui),
    );
    output.textures_delta.clear();
}

fn point(ctx: &egui::Context, key: &str) -> egui::Pos2 {
    ctx.data(|d| d.get_temp::<egui::Rect>(egui::Id::new(key)))
        .unwrap_or_else(|| panic!("Missing widget {key}"))
        .center()
}

fn pointer(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}

fn click(e: &mut Editor, ctx: &egui::Context, time: &mut f64, pos: egui::Pos2) {
    frame(
        e,
        ctx,
        time,
        vec![egui::Event::PointerMoved(pos), pointer(pos, true)],
    );
    frame(e, ctx, time, vec![pointer(pos, false)]);
}

fn drag(e: &mut Editor, ctx: &egui::Context, time: &mut f64, from: egui::Pos2, to: egui::Pos2) {
    frame(
        e,
        ctx,
        time,
        vec![egui::Event::PointerMoved(from), pointer(from, true)],
    );
    frame(
        e,
        ctx,
        time,
        vec![egui::Event::PointerMoved(from + vec2(15., 0.))],
    );
    frame(e, ctx, time, vec![egui::Event::PointerMoved(to)]);
    frame(e, ctx, time, vec![pointer(to, false)]);
}

fn key_event(key: egui::Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}
