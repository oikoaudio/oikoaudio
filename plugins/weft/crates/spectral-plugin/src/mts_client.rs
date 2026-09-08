//! Automatic global MTS-ESP tuning. Each client belongs to one thread at a time.
use oiko_dsp::note_frequency;
use spectral_dsp::MIDI_NOTES;
use std::ffi::{c_char, c_void};
use std::sync::atomic::{AtomicU32, Ordering::SeqCst};
static EQUAL_TEMPERAMENT: std::sync::LazyLock<[f32; MIDI_NOTES]> =
    std::sync::LazyLock::new(|| std::array::from_fn(|n| note_frequency(n as u8)));

unsafe extern "C" {
    fn MTS_RegisterClient() -> *mut c_void;
    fn MTS_DeregisterClient(client: *mut c_void);
    fn MTS_HasMaster(client: *mut c_void) -> bool;
    fn MTS_NoteToFrequency(client: *mut c_void, note: c_char, channel: i8) -> f64;
    fn MTS_ShouldFilterNote(client: *mut c_void, note: c_char, channel: i8) -> bool;
    fn MTS_GetScaleName(client: *mut c_void) -> *const c_char;
    fn MTS_GetMapSize(client: *mut c_void) -> i8;
    fn MTS_GetMapStartKey(client: *mut c_void) -> i8;
}

pub(crate) struct Client(*mut c_void);
// Ownership can move from construction to the audio thread. No shared access:
// every query requires &mut self because the C++ client caches query results.
unsafe impl Send for Client {}
impl Default for Client {
    fn default() -> Self {
        std::sync::LazyLock::force(&EQUAL_TEMPERAMENT);
        Self(unsafe { MTS_RegisterClient() })
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        unsafe { MTS_DeregisterClient(self.0) };
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Tuning {
    pub active: bool,
    pub frequencies: [f32; MIDI_NOTES],
    pub mapped: [bool; MIDI_NOTES],
    pub map_size: Option<usize>,
    pub map_start: usize,
}
impl Default for Tuning {
    fn default() -> Self {
        Self {
            active: false,
            frequencies: *EQUAL_TEMPERAMENT,
            mapped: [true; MIDI_NOTES],
            map_size: None,
            map_start: 0,
        }
    }
}
impl Client {
    pub fn read(&mut self) -> Tuning {
        let mut result = Tuning::default();
        if self.0.is_null() || !unsafe { MTS_HasMaster(self.0) } {
            return result;
        }
        result.active = true;
        for note in 0..MIDI_NOTES {
            // The global table keeps MPE expression channels and pinned notes in
            // the same tuning. Channel-specific tuning tables are not selected.
            let frequency = unsafe { MTS_NoteToFrequency(self.0, note as c_char, -1) };
            let valid = (frequency as f32).is_normal() && frequency > 0.0;
            result.mapped[note] =
                valid && !unsafe { MTS_ShouldFilterNote(self.0, note as c_char, -1) };
            if valid {
                result.frequencies[note] = frequency as f32;
            }
        }
        let size = unsafe { MTS_GetMapSize(self.0) };
        let start = unsafe { MTS_GetMapStartKey(self.0) };
        if size > 0 && start >= 0 {
            result.map_size = Some(size as usize);
            result.map_start = start as usize;
        }
        result
    }
    // Bounded, allocation-free copy. MTS names contain at most 255 bytes plus NUL.
    pub fn name_bytes(&mut self) -> [u8; 256] {
        let mut bytes = [0; 256];
        let name = unsafe { MTS_GetScaleName(self.0) };
        if !name.is_null() {
            for (i, byte) in bytes.iter_mut().take(255).enumerate() {
                *byte = unsafe { *name.add(i) } as u8;
                if *byte == 0 {
                    break;
                }
            }
        }
        bytes
    }
}
/// One audio writer, any number of UI readers. No locks, retries or allocation
/// in publish; a UI reader keeps its previous snapshot if publication overlaps.
pub(crate) struct Snapshot {
    generation: AtomicU32,
    words: [AtomicU32; 197],
}
impl Default for Snapshot {
    fn default() -> Self {
        let snapshot = Self {
            generation: AtomicU32::new(0),
            words: std::array::from_fn(|_| AtomicU32::new(0)),
        };
        snapshot.publish(&Tuning::default(), &[0; 256]);
        snapshot
    }
}
impl Snapshot {
    pub fn publish(&self, tuning: &Tuning, name: &[u8; 256]) {
        let mut words = [0; 197];
        for n in 0..128 {
            words[n] = tuning.frequencies[n].to_bits();
            if tuning.mapped[n] {
                words[128 + n / 32] |= 1 << (n % 32);
            }
        }
        words[132] = u32::from(tuning.active)
            | ((tuning.map_size.unwrap_or(0) as u32) << 8)
            | ((tuning.map_start as u32) << 16);
        for (word, bytes) in words[133..].iter_mut().zip(name.as_chunks::<4>().0.iter()) {
            *word = u32::from_le_bytes(*bytes);
        }
        self.generation.fetch_add(1, SeqCst);
        for (target, word) in self.words.iter().zip(words) {
            target.store(word, SeqCst);
        }
        self.generation.fetch_add(1, SeqCst);
    }
    pub fn read(&self) -> Option<(Tuning, String)> {
        let before = self.generation.load(SeqCst);
        if !before.is_multiple_of(2) {
            return None;
        }
        let words = self.words.each_ref().map(|word| word.load(SeqCst));
        if before != self.generation.load(SeqCst) {
            return None;
        }
        let size = ((words[132] >> 8) & 255) as usize;
        let tuning = Tuning {
            active: words[132] & 1 != 0,
            frequencies: std::array::from_fn(|n| f32::from_bits(words[n])),
            mapped: std::array::from_fn(|n| words[128 + n / 32] & (1 << (n % 32)) != 0),
            map_size: (size != 0).then_some(size),
            map_start: (words[132] >> 16) as usize,
        };
        let mut bytes = [0; 256];
        for (out, word) in bytes.as_chunks_mut::<4>().0.iter_mut().zip(&words[133..]) {
            out.copy_from_slice(&word.to_le_bytes());
        }
        let len = bytes.iter().position(|b| *b == 0).unwrap_or(256);
        Some((tuning, String::from_utf8_lossy(&bytes[..len]).into_owned()))
    }
}

impl Tuning {
    pub fn offset(&self, note: usize) -> f32 {
        if !self.active {
            return 0.0;
        }
        (12.0 * (self.frequencies[note] as f64 / EQUAL_TEMPERAMENT[note] as f64).log2()) as f32
    }
    pub fn boundary(&self, note: usize) -> bool {
        self.map_size.is_some_and(|size| {
            (note as isize - self.map_start as isize).rem_euclid(size as isize) == 0
        })
    }
    /// Geometric midpoints preserve frequency alignment on the log-frequency plot.
    /// Non-monotonic or duplicate mappings fall back to the normal MIDI keyboard.
    pub fn cells(&self) -> Option<Vec<(usize, f32, f32)>> {
        if !self.active {
            return None;
        }
        let notes: Vec<_> = (0..MIDI_NOTES).filter(|&n| self.mapped[n]).collect();
        if notes.len() < 2
            || notes
                .windows(2)
                .any(|w| self.frequencies[w[0]] >= self.frequencies[w[1]])
        {
            return None;
        }
        Some(
            notes
                .iter()
                .enumerate()
                .map(|(i, &note)| {
                    let center = self.frequencies[note] as f64;
                    let lower = if i > 0 {
                        (center * self.frequencies[notes[i - 1]] as f64).sqrt()
                    } else {
                        center / (self.frequencies[notes[1]] as f64 / center).sqrt()
                    };
                    let upper = if i + 1 < notes.len() {
                        (center * self.frequencies[notes[i + 1]] as f64).sqrt()
                    } else {
                        center * (center / self.frequencies[notes[i - 1]] as f64).sqrt()
                    };
                    (note, lower as f32, upper.min(f32::MAX as f64) as f32)
                })
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests;
