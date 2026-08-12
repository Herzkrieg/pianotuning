//! Platform-independent DSP and piano tuning logic.
//!
//! This crate contains everything needed to analyse audio, model string
//! inharmonicity, generate stretched tuning curves, apply historical
//! temperaments and persist tunings to disk. It has no Android or UI
//! dependencies so it can be unit tested on any host.

pub mod cents;
pub mod inharmonicity;
pub mod pitch_detection;
pub mod temperament;
pub mod tuning_curve;
pub mod tuning_file;

/// Number of keys on a standard piano (A0..C8).
pub const NUM_KEYS: usize = 88;

/// Key index of A4, the tuning reference key.
pub const A4_KEY_INDEX: usize = 48;
