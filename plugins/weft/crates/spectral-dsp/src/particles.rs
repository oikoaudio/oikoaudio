//! Deterministic spectral windows. The audio owner supplies resolved sources and
//! elapsed samples; preparation is the only operation that allocates.
use crate::MaskWorkspace;
use oiko_dsp::db_to_gain;

pub const MAX_PARTICLES: usize = 64;
/// Sparse harmonic subsets: at most three windows per Sprinkle gesture.
pub const WINDOWS_PER_PARTICLE: usize = 3;
pub const MAX_PARTIALS: usize = 24;
pub const PARTIAL_COUNT_CUMULATIVE: [f32; 2] = [0.80, 0.97];
pub const MAX_BIRTH_ATTEMPTS: usize = 64;
pub const MAX_SOURCES: usize = 256;
/// Separate keyed SplitMix64 streams: timing=0, source/register=1/2,
/// lifetime=3, retired movement key=4 (unused), Sprinkle grouping=5, retired attack key=6, width=7, free timing=8, retired brightness=9, opening=10, free root=11, partial count=12, weighted draws=13/14/15. Rejected opportunities still consume their index.
pub const SEED: u64 = 0x7765_6674_7061_7274;
pub const RETIRE_SECONDS: f64 = 0.02;
pub const TRANSITION_SECONDS: f32 = 0.05;
pub const LIFETIME_BASE: f32 = 0.2;
pub const LIFETIME_SIZE: f32 = 0.3;
pub const LIFETIME_VARIATION: f32 = 0.15;
/// Original audition design: three independent streams share one Rate budget.
/// Each stream has 0/1/2 starts (25/50/25%) per three Rate cycles: E[total]=Rate.
pub const SPRINKLE_STREAMS: usize = 3;
pub const SPRINKLE_GROUP_CYCLES: f64 = 3.0;
/// Keep zero-time stages continuous; spectral hops can extend the release further.
pub const SPRINKLE_MIN_STAGE_SECONDS: f32 = 0.002;
pub const SPRINKLE_DECAY_POWER: i32 = 3;
pub const SPRINKLE_WIDTH_RANGE: [f32; 2] = [0.65, 1.25];
/// Vary the window opening, never the selected background attenuation.
pub const SPRINKLE_OPENING_RANGE: [f32; 2] = [0.85, 1.0];
/// Random stream 11 keys phrase roots; free Sprinkle roots span three octaves; octave choices and partials share each root.
pub const FREE_ROOT_MIN_HZ: f32 = 200.0;
pub const FREE_ROOT_RATIO: f32 = 8.0;
pub const CLOUD_ATTACK: f32 = 0.5;
pub const REVERSE_ATTACK: f32 = 0.85;
pub const CLOUD_LIFETIME_SCALE: f32 = 2.0;
pub const PARTICLE_MIN_HZ: f32 = 20.0;
pub const SPRINKLE_MAX_HZ: f32 = 4186.01;
pub const PARTICLE_LOW_OPENING: f32 = 0.1;
/// Relative low-register selection weight; no frequency is excluded.
pub const PARTICLE_LOW_PROBABILITY: f32 = 0.15;
pub const PARTICLE_FULL_OPENING_HZ: f32 = 440.0;
pub const CLOUD_TIMING_SPREAD: f64 = 0.7;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Shape {
    #[default]
    Sprinkle,
    Cloud,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Direction {
    #[default]
    Forward,
    Reverse,
    Alternate,
}
#[derive(Clone, Copy, Debug)]
pub struct Config {
    pub enabled: bool,
    pub shape: Shape,
    pub direction: Direction,
    pub rate_hz: f32,
    pub size_octaves: f32,
    pub phase: f32,
    pub scheduled: bool,
    /// Quantize Sprinkle gestures to quarter subdivisions of the Rate cycle.
    pub sync: bool,
    pub hop_samples: usize,
    pub partials: usize,
    pub partial_rolloff_db: f32,
    /// Sprinkle onset and automatic decay after its peak, captured at birth.
    pub attack_ms: f32,
    pub release_ms: f32,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            shape: Shape::Sprinkle,
            direction: Direction::Forward,
            rate_hz: 1.0,
            size_octaves: 1.0,
            phase: 0.0,
            scheduled: true,
            sync: false,
            hop_samples: 2048,
            partials: 1,
            partial_rolloff_db: 6.0,
            attack_ms: 0.0,
            release_ms: 70.0,
        }
    }
}
impl Config {
    /// Capture duration in seconds and the attack fraction for one new event.
    fn envelope(&self, index: u64, sample_rate: f32) -> (f32, f32) {
        let size = self.size_octaves;
        let rate = self.rate_hz;
        let sprinkle = self.shape == Shape::Sprinkle;
        let hop_floor = 2.0 * self.hop_samples as f32 / sample_rate;
        if sprinkle {
            // The user's millisecond settings own the contour. Rate changes
            // births and Size changes width, without rescaling either stage.
            let onset = (self.attack_ms * 0.001).max(SPRINKLE_MIN_STAGE_SECONDS);
            let release = (self.release_ms * 0.001)
                .max(SPRINKLE_MIN_STAGE_SECONDS)
                .max(hop_floor - onset);
            let lifetime = onset + release;
            (lifetime, onset / lifetime)
        } else {
            let minimum = 0.06_f32.max(hop_floor);
            let variation = 1.0 + LIFETIME_VARIATION * (2.0 * random(index, 3) - 1.0);
            let lifetime = (CLOUD_LIFETIME_SCALE * (LIFETIME_BASE + LIFETIME_SIZE * size.sqrt())
                / rate.sqrt()
                * variation)
                .clamp(minimum, 4.0_f32.max(minimum));
            let attack = match self.direction {
                Direction::Forward => CLOUD_ATTACK,
                Direction::Reverse => REVERSE_ATTACK,
                Direction::Alternate if index & 1 == 0 => CLOUD_ATTACK,
                Direction::Alternate => REVERSE_ATTACK,
            };
            (lifetime, attack)
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Source {
    pub frequency_hz: f32,
    /// Resolved velocity sensitivity, bounded to unity. Expression gain/pressure
    /// are applied only by the ordinary note mask, not a second time here.
    pub strength: f32,
    pub eligible: bool,
}
impl Source {
    fn eligible(self) -> bool {
        self.eligible
            && self.frequency_hz.is_finite()
            && self.frequency_hz > 0.0
            && self.strength.is_finite()
            && self.strength > 0.0
    }
}
#[derive(Clone, Copy, Debug, Default)]
struct Particle {
    active: bool,
    source: Option<usize>,
    birth: u64,
    lifetime: f64,
    frequency_hz: f32,
    octave: f32,
    width: f32,
    strength: f32,
    attack: f32,
    decay_power: i32,
    /// Selected harmonic numbers; zero marks an unused window. Cloud uses [1, 0, 0].
    partials: [u8; WINDOWS_PER_PARTICLE],
    retirement: Option<u64>,
}
/// A frame observation, evaluated by the very same window function as audio.
#[derive(Clone, Copy, Debug, Default)]
pub struct Window {
    pub center_log2_hz: f32,
    pub radius_octaves: f32,
    pub amplitude: f32,
}
impl Window {
    pub fn openness(self, log2_hz: f32) -> f32 {
        if self.radius_octaves <= 0.0 {
            return 0.0;
        }
        let x = (1.0 - (log2_hz - self.center_log2_hz).abs() / self.radius_octaves).clamp(0.0, 1.0);
        self.amplitude * taper(x)
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub windows: [Window; MAX_PARTICLES * WINDOWS_PER_PARTICLE],
    pub activity: f32,
}
impl Default for Frame {
    fn default() -> Self {
        Self {
            windows: [Window::default(); MAX_PARTICLES * WINDOWS_PER_PARTICLE],
            activity: 0.0,
        }
    }
}
impl Frame {
    pub fn attenuation_db(&self, frequency_hz: f32, depth_db: f32) -> f32 {
        if depth_db <= 0.0 {
            return 0.0;
        }
        let log = frequency_hz.max(f32::MIN_POSITIVE).log2();
        let openness = self
            .windows
            .iter()
            .fold(0.0_f32, |o, w| o.max(w.openness(log)));
        depth_db.clamp(0.0, 60.0) * self.activity * (1.0 - openness)
    }
}

/// Stream-local indices occupy disjoint keys below the immediate-note namespace.
fn sprinkle_key(lane: usize, index: u64) -> u64 {
    (1 << 62) | ((lane as u64) << 59) | index
}

#[derive(Clone, Copy)]
struct SprinkleStream {
    index: u64,
    next: f64,
    emit: bool,
}
impl SprinkleStream {
    fn new(lane: usize, sync: bool) -> Self {
        let mut stream = Self {
            index: 0,
            next: 0.0,
            emit: false,
        };
        stream.prepare(lane, sync);
        stream
    }
    fn prepare(&mut self, lane: usize, sync: bool) {
        let group = self.index / 2;
        let key = sprinkle_key(lane, group * 2);
        let count_draw = random(key, 5);
        let count = if count_draw < 0.25 {
            0
        } else if count_draw < 0.75 {
            1
        } else {
            2
        };
        self.emit = self.index % 2 < count;
        // First starts lie on a quarter-cycle grid; the second is simultaneous
        // or follows by a quarter/half cycle. Free adds a small shared group offset.
        let start = 0.25 + (random(key, 0) * 9.0).floor() as f64 * 0.25;
        let gap = (random(key + 1, 0) * 3.0).floor() as f64 * 0.25;
        let human = if sync {
            0.0
        } else {
            0.18 * (random(key, 8) as f64 - 0.5)
        };
        self.next =
            group as f64 * SPRINKLE_GROUP_CYCLES + start + (self.index % 2) as f64 * gap + human;
    }
}

/// Low-register defaults are selection/strength weights, never a mask cutoff.
fn register_height(log_hz: f32) -> f32 {
    ((log_hz - PARTICLE_MIN_HZ.log2()) / (PARTICLE_FULL_OPENING_HZ / PARTICLE_MIN_HZ).log2())
        .clamp(0.0, 1.0)
}
fn register_strength(log_hz: f32) -> f32 {
    let height = register_height(log_hz);
    PARTICLE_LOW_OPENING + (1.0 - PARTICLE_LOW_OPENING) * height * height
}
fn register_probability(log_hz: f32) -> f32 {
    PARTICLE_LOW_PROBABILITY + (1.0 - PARTICLE_LOW_PROBABILITY) * register_height(log_hz)
}

/// Integrate a linearly rising probability density in log frequency, then a
/// flat density above 440 Hz. Invert it directly: bounded work, no rejection loop.
fn free_cloud_frequency(index: u64, max_hz: f32) -> f32 {
    let lower = PARTICLE_MIN_HZ.min(max_hz * 0.5).log2();
    let span = max_hz.log2() - lower;
    let ramp = (PARTICLE_FULL_OPENING_HZ.log2() - lower).max(f32::EPSILON);
    let slope = (1.0 - PARTICLE_LOW_PROBABILITY) / ramp;
    let end = span.min(ramp);
    let ramp_mass = PARTICLE_LOW_PROBABILITY * end + 0.5 * slope * end * end;
    let target = random(index, 1) * (ramp_mass + (span - ramp).max(0.0));
    let offset = if target <= ramp_mass {
        2.0 * target
            / (PARTICLE_LOW_PROBABILITY
                + (PARTICLE_LOW_PROBABILITY.powi(2) + 2.0 * slope * target).sqrt())
    } else {
        ramp + target - ramp_mass
    };
    (lower + offset).exp2()
}

/// Choose an octave-related register across 20–4186 Hz. Low registers remain
/// possible but less frequent; Direction adds a modest tilt across the range.
fn sprinkle_register(index: u64, root_hz: f32, max_hz: f32, direction: Direction) -> (f32, f32) {
    let upper = SPRINKLE_MAX_HZ.min(max_hz.max(1.0));
    let lower = PARTICLE_MIN_HZ.min(upper * 0.5);
    let root_log = root_hz.log2();
    let first = (lower.log2() - root_log).ceil();
    let last = (upper.log2() - root_log).floor().max(first);
    let count = (last - first + 1.0) as u32;
    let upward = match direction {
        Direction::Forward => true,
        Direction::Reverse => false,
        Direction::Alternate => index & 1 == 0,
    };
    let weight = |i: u32| {
        let position = i as f32 / (count - 1).max(1) as f32;
        let tilt = if upward {
            0.5 + position
        } else {
            1.5 - position
        };
        register_probability(root_log + first + i as f32) * tilt
    };
    let total: f32 = (0..count).map(weight).sum();
    let mut target = random(index, 2) * total;
    let mut chosen = count - 1;
    for i in 0..count {
        target -= weight(i);
        if target < 0.0 {
            chosen = i;
            break;
        }
    }
    let octave = first + chosen as f32;
    (octave, register_strength(root_log + octave))
}

/// Bounded weighted sampling without replacement. Rolloff affects the likelihood
/// of choosing a harmonic, never its gain. Keys do not depend on other births.
fn select_partials(
    index: u64,
    mut weights: [f32; MAX_PARTIALS],
    limit: usize,
    max_ratio: f32,
) -> [u8; WINDOWS_PER_PARTICLE] {
    let draw = random(index, 12);
    let count = if draw < PARTIAL_COUNT_CUMULATIVE[0] {
        1
    } else if draw < PARTIAL_COUNT_CUMULATIVE[1] {
        2
    } else {
        3
    };
    for (i, weight) in weights.iter_mut().enumerate() {
        if i >= limit || (i + 1) as f32 >= max_ratio {
            *weight = 0.0;
        }
    }
    let mut selected = [0; WINDOWS_PER_PARTICLE];
    for (slot, partial) in selected.iter_mut().enumerate().take(count) {
        let total: f32 = weights.iter().sum();
        if total <= 0.0 {
            break;
        }
        let mut target = random(index, 13 + slot as u64) * total;
        let mut chosen = None;
        for (i, weight) in weights.iter().copied().enumerate() {
            if weight <= 0.0 {
                continue;
            }
            chosen = Some(i);
            if target < weight {
                break;
            }
            target -= weight;
        }
        if let Some(i) = chosen {
            *partial = (i + 1) as u8;
            weights[i] = 0.0;
        }
    }
    selected
}

/// Immutable sample-rate data prepared once and retained across audio resets.
#[derive(Clone, Copy)]
struct Prepared {
    sample_rate: f64,
    phase_alpha: f64,
    partial_logs: [f32; MAX_PARTIALS],
    default_partial_weights: [f32; MAX_PARTIALS],
}

pub struct Engine {
    particles: [Particle; MAX_PARTICLES],
    sources: [Source; MAX_SOURCES],
    prepared: Prepared,
    sample: u64,
    phase: f64,
    phase_offset: f64,
    opportunity: u64,
    immediate_index: u64,
    streams: [SprinkleStream; SPRINKLE_STREAMS],
    last_source: Option<usize>,
    note_input_present: bool,
    partial_weights: [f32; MAX_PARTIALS],
    attempts: usize,
    config: Config,
    activity: f32,
    /// Diagnostic counters/hash contain decisions only, not audio or RNG state.
    pub births: u64,
    pub dropped: u64,
    pub decision_hash: u64,
}
impl Engine {
    pub fn new(sample_rate: f32) -> Self {
        assert!(sample_rate.is_finite() && sample_rate > 0.0);
        let partial_logs = std::array::from_fn(|i| ((i + 1) as f32).log2());
        Self::from_prepared(Prepared {
            sample_rate: sample_rate as f64,
            phase_alpha: 1.0 - (-1.0 / (0.05 * sample_rate as f64)).exp(),
            partial_logs,
            default_partial_weights: partial_logs
                .map(|log| db_to_gain(-Config::default().partial_rolloff_db * log)),
        })
    }
    fn from_prepared(prepared: Prepared) -> Self {
        Self {
            particles: [Particle::default(); MAX_PARTICLES],
            sources: [Source::default(); MAX_SOURCES],
            prepared,
            sample: 0,
            phase: 0.0,
            phase_offset: 0.0,
            opportunity: 0,
            immediate_index: 0,
            streams: std::array::from_fn(|lane| SprinkleStream::new(lane, false)),
            last_source: None,
            note_input_present: false,
            partial_weights: prepared.default_partial_weights,
            attempts: 0,
            config: Config::default(),
            activity: 0.0,
            births: 0,
            dropped: 0,
            decision_hash: 0,
        }
    }
    pub fn reset(&mut self) {
        *self = Self::from_prepared(self.prepared);
    }
    pub fn configure(&mut self, mut config: Config) {
        config.attack_ms = finite(config.attack_ms, 0.0).clamp(0.0, 5000.0);
        config.release_ms = finite(config.release_ms, 70.0).clamp(0.0, 5000.0);
        config.partials = config.partials.clamp(1, MAX_PARTIALS);
        config.partial_rolloff_db = finite(config.partial_rolloff_db, 6.0).clamp(0.0, 24.0);
        if config.partial_rolloff_db != self.config.partial_rolloff_db {
            self.partial_weights = self
                .prepared
                .partial_logs
                .map(|log| db_to_gain(-config.partial_rolloff_db * log));
        }
        config.rate_hz = finite(config.rate_hz, 1.0).clamp(0.01, 32.0);
        config.size_octaves = finite(config.size_octaves, 1.0).clamp(0.125, 8.0);
        config.phase = finite(config.phase, 0.0).rem_euclid(1.0);
        if config.enabled != self.config.enabled || config.shape != self.config.shape {
            self.retire_all();
        }
        if self.sample == 0 {
            self.phase_offset = config.phase as f64;
        }
        let enter_sprinkle = config.shape == Shape::Sprinkle
            && (self.config.shape != config.shape || (!self.config.enabled && config.enabled));
        let sync_changed = self.config.sync != config.sync;
        self.config = config;
        if enter_sprinkle {
            self.align_streams();
        } else if sync_changed {
            for (lane, stream) in self.streams.iter_mut().enumerate() {
                stream.prepare(lane, config.sync);
            }
        }
    }
    /// Presence includes muted or unmapped notes: they must not enable free
    /// fallback merely because no audible source could be resolved.
    pub fn set_note_input_present(&mut self, present: bool) {
        self.note_input_present = present;
    }
    pub fn set_source(&mut self, slot: usize, source: Source) {
        if slot >= MAX_SOURCES {
            return;
        }
        if self.sources[slot] == source {
            return;
        }
        self.sources[slot] = source;
        if source.frequency_hz.is_finite() && source.frequency_hz > 0.0 {
            for p in &mut self.particles {
                if p.active && p.source == Some(slot) {
                    p.frequency_hz = source.frequency_hz;
                }
            }
        }
    }
    /// Called before reuse/unmapping/choke. Detaching identity prevents a reused
    /// host slot from retuning the retiring particle.
    pub fn retire_source(&mut self, slot: usize) {
        self.sources[slot] = Source::default();
        for p in &mut self.particles {
            if p.active && p.source == Some(slot) {
                p.source = None;
                p.retirement.get_or_insert(self.sample);
            }
        }
    }
    /// A natural release stops this source supplying births; its windows finish at their last pitch.
    pub fn release_source(&mut self, slot: usize) {
        self.sources[slot].eligible = false;
    }
    pub fn trigger(&mut self, slot: usize) {
        let index = self.reserve_trigger();
        self.trigger_reserved(slot, index);
    }
    /// Reserve at note arrival, even if same-sample lifecycle later discards it.
    /// Overflow or coalescing must not shift later note variation streams.
    pub fn reserve_trigger(&mut self) -> u64 {
        let index = self.immediate_index;
        self.immediate_index = self.immediate_index.wrapping_add(1);
        index
    }
    pub fn trigger_reserved(&mut self, slot: usize, index: u64) {
        if slot < MAX_SOURCES
            && self.config.enabled
            && self.config.shape == Shape::Sprinkle
            && self.sources[slot].eligible()
        {
            self.birth(index ^ (1 << 63), Some(slot));
        }
    }
    fn retire_all(&mut self) {
        for p in &mut self.particles {
            if p.active {
                p.source = None;
                p.retirement.get_or_insert(self.sample);
            }
        }
    }
    /// A seek starts an epoch at the destination, never reconstructing missed history.
    pub fn seek(&mut self, cycles: f64) {
        self.retire_all();
        self.phase = if cycles.is_finite() {
            cycles.rem_euclid(1_000_000_000.0)
        } else {
            0.0
        };
        self.opportunity = self.phase.floor() as u64;
        self.phase_offset = self.config.phase as f64;
        self.immediate_index = 0;
        self.last_source = None;
        self.align_streams();
        if self.timing_reference() <= self.phase + self.phase_offset {
            self.opportunity += 1;
        }
    }
    fn align_streams(&mut self) {
        let reference = self.phase + self.phase_offset;
        for (lane, stream) in self.streams.iter_mut().enumerate() {
            stream.index = 2 * (reference / SPRINKLE_GROUP_CYCLES).floor().max(0.0) as u64;
            stream.prepare(lane, self.config.sync);
            // At most two opportunities in the destination group; discard history.
            for _ in 0..2 {
                if stream.next <= reference {
                    stream.index += 1;
                    stream.prepare(lane, self.config.sync);
                }
            }
        }
    }
    pub fn elapsed_samples(&self) -> u64 {
        self.sample
    }
    pub fn active_count(&self) -> usize {
        self.particles.iter().filter(|p| p.active).count()
    }
    /// Sample recurrence makes timing and phase automation independent of slices.
    /// The loop is bounded by supplied audio samples; at most 64 births per hop.
    pub fn advance(&mut self, samples: usize) {
        let step = self.config.rate_hz as f64 / self.prepared.sample_rate;
        let alpha = self.prepared.phase_alpha;
        for _ in 0..samples {
            // Deliver births before this sample, after the adapter has applied
            // lifecycle and expression events at the same offset.
            if self.config.scheduled {
                let reference = self.phase + self.phase_offset;
                if self.config.shape == Shape::Sprinkle {
                    for lane in 0..SPRINKLE_STREAMS {
                        // A pair may deliberately start together. Never retain a backlog.
                        for _ in 0..2 {
                            let stream = self.streams[lane];
                            if reference < stream.next {
                                break;
                            }
                            self.streams[lane].index = (stream.index + 1).max(
                                2 * (reference / SPRINKLE_GROUP_CYCLES).floor().max(0.0) as u64,
                            );
                            self.streams[lane].prepare(lane, self.config.sync);
                            if stream.emit && self.config.enabled {
                                self.birth(sprinkle_key(lane, stream.index), None);
                            }
                        }
                    }
                } else if reference >= self.timing_reference() {
                    let index = self.opportunity;
                    self.opportunity = (index + 1).max(reference.floor().max(0.0) as u64);
                    if self.config.enabled {
                        self.birth(index, None);
                    }
                }
                self.phase += step;
            }
            let difference =
                (self.config.phase as f64 - self.phase_offset + 0.5).rem_euclid(1.0) - 0.5;
            self.phase_offset += difference * alpha;
            self.sample += 1;
        }
    }
    fn timing_reference(&self) -> f64 {
        self.opportunity as f64
            + 0.5
            + CLOUD_TIMING_SPREAD * (random(self.opportunity, 0) as f64 - 0.5)
    }
    fn birth(&mut self, index: u64, source: Option<usize>) {
        let source = {
            let eligible = |(i, s): &(usize, &Source)| s.eligible() && Some(*i) != self.last_source;
            let chosen = source.or_else(|| {
                let alternatives = self.sources.iter().enumerate().filter(eligible).count();
                if alternatives == 0 {
                    self.sources.iter().position(|s| s.eligible())
                } else {
                    let choice =
                        ((random(index, 1) * alternatives as f32) as usize).min(alternatives - 1);
                    self.sources
                        .iter()
                        .enumerate()
                        .filter(eligible)
                        .nth(choice)
                        .map(|(i, _)| i)
                }
            });
            // Update selection history even for rejected births: overflow cannot
            // change the source sequence after the pool becomes available again.
            self.last_source = chosen;
            chosen
        };
        if self.attempts == MAX_BIRTH_ATTEMPTS {
            self.dropped += 1;
            return;
        }
        self.attempts += 1;
        if source.is_none() && self.note_input_present {
            return;
        }
        let Some(p) = self.particles.iter_mut().find(|p| !p.active) else {
            self.dropped += 1;
            return;
        };
        let size = self.config.size_octaves;
        let sprinkle = self.config.shape == Shape::Sprinkle;
        let (lifetime, attack) = self
            .config
            .envelope(index, self.prepared.sample_rate as f32);
        let max_hz = (self.prepared.sample_rate as f32 * 0.5).min(20_000.0);
        let frequency_hz = source.map_or_else(
            || {
                if sprinkle {
                    // A shared root for three Rate cycles gives the free streams
                    // a short harmonic phrase. Phase offset does not change its key.
                    let phrase = (self.phase / SPRINKLE_GROUP_CYCLES).floor() as u64;
                    FREE_ROOT_MIN_HZ * FREE_ROOT_RATIO.powf(random(phrase, 11))
                } else {
                    // Direction selects the envelope; free placement gently favors higher registers.
                    free_cloud_frequency(index, max_hz)
                }
            },
            |s| self.sources[s].frequency_hz,
        );
        let (octave, register_strength) = if sprinkle {
            sprinkle_register(index, frequency_hz, max_hz, self.config.direction)
        } else {
            // Deliberate low note input stays at its resolved pitch.
            (0.0, register_strength(frequency_hz.log2()))
        };
        *p = Particle {
            active: true,
            source,
            birth: self.sample,
            lifetime: lifetime as f64,
            frequency_hz,
            octave,
            width: size
                * 0.5
                * if sprinkle {
                    SPRINKLE_WIDTH_RANGE[0]
                        + (SPRINKLE_WIDTH_RANGE[1] - SPRINKLE_WIDTH_RANGE[0]) * random(index, 7)
                } else {
                    1.0
                },
            strength: source.map_or(1.0, |s| self.sources[s].strength.clamp(0.0, 1.0))
                * register_strength
                * if sprinkle {
                    SPRINKLE_OPENING_RANGE[0]
                        + (SPRINKLE_OPENING_RANGE[1] - SPRINKLE_OPENING_RANGE[0])
                            * random(index, 10)
                } else {
                    1.0
                },
            attack,
            decay_power: if sprinkle { SPRINKLE_DECAY_POWER } else { 1 },
            partials: if sprinkle {
                select_partials(
                    index,
                    self.partial_weights,
                    self.config.partials,
                    max_hz / (frequency_hz.log2() + octave).exp2(),
                )
            } else {
                [1, 0, 0]
            },
            retirement: None,
        };
        self.births += 1;
        self.decision_hash = mix(self.decision_hash
            ^ index
            ^ self.sample.rotate_left(19)
            ^ (frequency_hz.to_bits() as u64)
            ^ u64::from_le_bytes([p.partials[0], p.partials[1], p.partials[2], 0, 0, 0, 0, 0]));
    }
    /// Evaluate once at the actual spectral boundary, then renew the hop budget.
    pub fn frame(&mut self, bin_hz: f32, elapsed_seconds: f32) -> Frame {
        let mut frame = Frame::default();
        for (slot, p) in self.particles.iter_mut().enumerate() {
            if !p.active {
                continue;
            }
            let age = (self.sample - p.birth) as f64 / self.prepared.sample_rate;
            let retirement = p.retirement.map_or(0.0, |sample| {
                (self.sample - sample) as f64 / self.prepared.sample_rate / RETIRE_SECONDS
            });
            if age >= p.lifetime || retirement >= 1.0 {
                *p = Particle::default();
                continue;
            }
            let x = (age / p.lifetime) as f32;
            let attack = p.attack;
            let envelope = if x < attack {
                taper(x / attack)
            } else {
                taper((1.0 - x) / (1.0 - attack)).powi(p.decay_power)
            };
            let root_log = p.frequency_hz.log2() + p.octave;
            let amplitude = p.strength * envelope * taper(1.0 - retirement as f32);
            for (window, partial) in p.partials.iter().copied().enumerate() {
                if partial == 0 {
                    continue;
                }
                let center = root_log + self.prepared.partial_logs[partial as usize - 1];
                let hz = center.exp2();
                if hz >= (self.prepared.sample_rate as f32 * 0.5).min(20_000.0) {
                    continue;
                }
                frame.windows[slot + window * MAX_PARTICLES] = Window {
                    center_log2_hz: center,
                    radius_octaves: p.width.max((1.0 + bin_hz.max(0.0) / hz.max(1.0)).log2()),
                    amplitude,
                };
            }
        }
        // Motion Depth sets the background even without note sources. Releasing
        // the last Sprinkle source closes its windows; it must never bypass the
        // motion layer and expose the incoming audio at full level.
        let active = self.config.enabled;
        let target = f32::from(active);
        self.activity +=
            (target - self.activity) * (1.0 - (-elapsed_seconds / TRANSITION_SECONDS).exp());
        if !active && self.activity < 1e-6 {
            self.activity = 0.0;
        }
        frame.activity = self.activity;
        self.attempts = 0;
        frame
    }
}

/// Prepared scratch uses compact-support ranges and polynomial tapers, so the
/// bins × particles loop has no transcendental functions or allocations.
pub struct Mask {
    openness: Vec<f32>,
}
impl Mask {
    pub fn new(bins: usize) -> Self {
        Self {
            openness: vec![0.0; bins],
        }
    }
    pub fn gains(
        &mut self,
        frame: &Frame,
        depth_db: f32,
        workspace: &MaskWorkspace,
        output: &mut [f32],
    ) {
        assert_eq!(output.len(), self.openness.len());
        assert_eq!(output.len(), workspace.bin_log2_hz.len());
        if depth_db <= 0.0 || frame.activity == 0.0 {
            output.fill(1.0);
            return;
        }
        self.openness.fill(0.0);
        for w in &frame.windows {
            if w.amplitude <= 0.0 {
                continue;
            }
            let logs = &workspace.bin_log2_hz;
            let start = logs
                .partition_point(|f| *f < w.center_log2_hz - w.radius_octaves)
                .max(1);
            let end = logs.partition_point(|f| *f <= w.center_log2_hz + w.radius_octaves);
            for (o, log) in self.openness[start.min(end)..end]
                .iter_mut()
                .zip(&logs[start.min(end)..end])
            {
                *o = o.max(w.openness(*log));
            }
        }
        for (gain, openness) in output.iter_mut().zip(&self.openness) {
            *gain = db_to_gain(-depth_db.clamp(0.0, 60.0) * frame.activity * (1.0 - openness));
        }
    }
}
fn taper(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}
fn finite(x: f32, fallback: f32) -> f32 {
    if x.is_finite() { x } else { fallback }
}
fn mix(mut x: u64) -> u64 {
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}
fn random(index: u64, stream: u64) -> f32 {
    (mix(SEED
        ^ index.wrapping_mul(0x9e37_79b9_7f4a_7c15)
        ^ stream.wrapping_mul(0xd1b5_4a32_d192_ed03))
        >> 40) as f32
        / 16_777_216.0
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod measurements;
