//! Extract an explicitly chosen SCL degree range. Originals are never changed.
//! Usage: extract_cycle source.scl start_degree note_count destination.scl
use std::{io::Write, path::Path};
fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 4 {
        return Err("Pass source.scl, start degree, note count, destination.scl".into());
    }
    let source = Path::new(&args[0]);
    let destination = Path::new(&args[3]);
    let name = destination
        .file_stem()
        .ok_or("Missing destination name")?
        .to_string_lossy()
        .to_string();
    let preset = inton_core::tuning::Preset::new(
        source.file_stem().unwrap().to_string_lossy().into(),
        std::fs::read_to_string(source).map_err(|e| e.to_string())?,
        std::fs::read_to_string(source.with_extension("kbm")).ok(),
    );
    let mut draft = inton_core::scale_edit::Draft::new(preset)?;
    let start = args[1].parse().map_err(|_| "Invalid start")?;
    let count = args[2].parse().map_err(|_| "Invalid count")?;
    draft.extract_cycle(start, count)?;
    draft.name = name;
    let p = draft.preset()?;
    let t = p.prepare()?;
    std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(destination)
        .and_then(|mut f| f.write_all(p.scl_text.as_bytes()))
        .map_err(|e| e.to_string())?;
    println!(
        "{}: {} notes, {:.6} cents, MIDI69 {:.3} Hz",
        destination.display(),
        t.count,
        1200. * t.period.log2(),
        t.hz[69]
    );
    Ok(())
}
