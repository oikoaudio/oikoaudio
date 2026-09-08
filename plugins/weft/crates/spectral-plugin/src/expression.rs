//! Host input adapters. MIDI channel/zone semantics stop at resolved VoiceExpression.
use super::{MAX_VOICES, VoiceState};

#[derive(Clone, Copy, Debug)]
pub(crate) struct VoiceExpression {
    pub tuning_semitones: f32,
    pub gain: f32,
    pub pan: f32,
    pub timbre: f32,
    pub pressure: f32,
}
impl VoiceExpression {
    pub const DEFAULT: Self = Self {
        tuning_semitones: 0.0,
        gain: 1.0,
        pan: 0.0,
        timbre: 0.5,
        pressure: 1.0,
    };
}

#[derive(Clone, Copy)]
struct Channel {
    bend: f32,
    pressure: f32,
    timbre: f32,
    bend_range: Option<f32>,
    rpn: [u8; 2],
    data: [u8; 2],
}
impl Channel {
    const DEFAULT: Self = Self {
        bend: 0.0,
        pressure: 1.0,
        timbre: 0.5,
        bend_range: None,
        rpn: [127; 2],
        data: [0; 2],
    };
}

pub(crate) struct MidiExpression {
    channels: [Channel; 16],
    lower_members: u8,
    upper_members: u8,
}
impl Default for MidiExpression {
    fn default() -> Self {
        Self {
            channels: [Channel::DEFAULT; 16],
            lower_members: 0,
            upper_members: 0,
        }
    }
}
impl MidiExpression {
    pub fn master_for(&self, channel: u8) -> Option<u8> {
        if channel > 0 && channel <= self.lower_members {
            Some(0)
        } else if channel < 15 && channel >= 15 - self.upper_members {
            Some(15)
        } else {
            None
        }
    }
    pub fn is_master(&self, channel: u8) -> bool {
        (channel == 0 && self.lower_members > 0) || (channel == 15 && self.upper_members > 0)
    }
    pub fn bend(&self, channel: u8, fallback_range: f32) -> f32 {
        let state = &self.channels[channel as usize];
        let range = state.bend_range.unwrap_or(if self.is_master(channel) {
            2.0
        } else {
            fallback_range
        });
        state.bend * range
            + self
                .master_for(channel)
                .map_or(0.0, |master| self.bend(master, fallback_range))
    }
    pub fn pitch_bend(&mut self, channel: u8, value: f32) {
        if channel < 16 && value.is_finite() {
            // Raw MIDI's exact center is 8192/16383; hosts may also send exactly 0.5.
            let centered = if (value - 0.5).abs() <= 0.5 / 16383.0 + f32::EPSILON {
                0.0
            } else {
                (value.clamp(0.0, 1.0) - 0.5) * 2.0
            };
            self.channels[channel as usize].bend = centered;
        }
    }
    pub fn pressure(&mut self, channel: u8, value: f32) {
        if channel < 16 && value.is_finite() {
            self.channels[channel as usize].pressure = value.clamp(0.0, 1.0);
        }
    }
    pub fn cc(&mut self, channel: u8, cc: u8, value: f32) {
        if channel >= 16 || !value.is_finite() {
            return;
        }
        let byte = (value.clamp(0.0, 1.0) * 127.0).round() as u8;
        let old_memberships: [Option<u8>; 16] = std::array::from_fn(|c| self.zone_for(c as u8));
        let mut new_range = None;
        let mut configured_master = None;
        let state = &mut self.channels[channel as usize];
        match cc {
            74 => state.timbre = value.clamp(0.0, 1.0),
            101 => {
                state.rpn[0] = byte;
                state.data = [0; 2];
            }
            100 => {
                state.rpn[1] = byte;
                state.data = [0; 2];
            }
            99 | 98 => state.rpn = [127; 2],
            6 | 38 => {
                state.data[usize::from(cc == 38)] = byte;
                if state.rpn == [0, 0] {
                    new_range = Some(
                        (state.data[0] as f32 + state.data[1].min(99) as f32 * 0.01).min(96.0),
                    );
                    state.bend_range = new_range;
                } else if state.rpn == [0, 6] && cc == 6 {
                    // A newly configured zone wins; keep lower and upper memberships disjoint.
                    if channel == 0 && byte <= 15 {
                        self.lower_members = byte;
                        if byte > 0 {
                            self.upper_members = self.upper_members.min(14u8.saturating_sub(byte));
                        }
                        configured_master = Some(0);
                    } else if channel == 15 && byte <= 15 {
                        self.upper_members = byte;
                        if byte > 0 {
                            self.lower_members = self.lower_members.min(14u8.saturating_sub(byte));
                        }
                        configured_master = Some(15);
                    }
                }
            }
            121 => {
                state.bend = 0.0;
                state.pressure = 1.0;
                state.timbre = 0.5;
                state.rpn = [127; 2];
            }
            _ => {}
        }
        if let Some(range) = new_range
            && let Some(master) = self.master_for(channel)
        {
            for member in 0..16 {
                if self.master_for(member) == Some(master) {
                    self.channels[member as usize].bend_range = Some(range);
                }
            }
        }
        if let Some(master) = configured_master {
            for (channel, old) in old_memberships.into_iter().enumerate() {
                let zone = self.zone_for(channel as u8);
                if old != zone {
                    self.channels[channel] = Channel::DEFAULT;
                }
                if zone == Some(master) {
                    self.channels[channel].bend_range =
                        Some(if channel as u8 == master { 2.0 } else { 48.0 });
                }
            }
        }
    }
    pub fn zone_for(&self, channel: u8) -> Option<u8> {
        if self.is_master(channel) {
            Some(channel)
        } else {
            self.master_for(channel)
        }
    }
    pub fn resolve(&self, voices: &mut [VoiceState; MAX_VOICES], fallback_range: f32) {
        for voice in voices.iter_mut().filter(|v| v.occupied) {
            let channel = &self.channels[voice.channel as usize];
            let master = self
                .master_for(voice.channel)
                .map(|c| &self.channels[c as usize]);
            voice.expression = VoiceExpression {
                tuning_semitones: voice.tuning_semitones + self.bend(voice.channel, fallback_range),
                gain: voice.volume_gain * voice.expression_amount,
                pan: voice.pan,
                // Native values take precedence over MIDI compatibility state for Y/Z.
                pressure: voice
                    .native_pressure
                    .unwrap_or(channel.pressure * master.map_or(1.0, |m| m.pressure)),
                timbre: voice.native_timbre.unwrap_or(
                    (channel.timbre + master.map_or(0.0, |m| m.timbre - 0.5)).clamp(0.0, 1.0),
                ),
            };
        }
    }
}
