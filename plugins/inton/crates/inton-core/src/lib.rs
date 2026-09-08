//! Inton tuning, project state, and MTS control runtime, independent of GUI and host formats.
//! Parsing, state edits, and MTS calls run off the audio thread. Only the bounded
//! parameter mailbox is designed to be used by an audio callback.
pub mod engine;
pub mod mts;
pub mod runtime;
pub mod scale_edit;
pub mod state;
pub mod tuning;
