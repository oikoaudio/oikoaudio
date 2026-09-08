// Read-only diagnostic; pass local SCL paths. Does not start an MTS master.
fn main() {
    for name in std::env::args().skip(1) {
        let path = std::path::Path::new(&name);
        let kbm = path.with_extension("kbm");
        let preset = inton_core::tuning::Preset::new(
            name.clone(),
            std::fs::read_to_string(path).unwrap(),
            std::fs::read_to_string(kbm).ok(),
        );
        match preset.prepare() {
            Ok(t) => {
                println!(
                    "{}: {} degrees, reference {}, period {:.3}c; Hz min {:.3}, max {:.3}, MIDI60 {:.3}, MIDI69 {:.3}",
                    path.file_stem().unwrap().to_string_lossy(),
                    t.count,
                    t.reference_note,
                    1200. * t.period.log2(),
                    t.hz.iter().copied().fold(f64::INFINITY, f64::min),
                    t.hz.iter().copied().fold(0., f64::max),
                    t.hz[60],
                    t.hz[69]
                );
            }
            Err(e) => println!("{name}: {e}"),
        }
    }
}
