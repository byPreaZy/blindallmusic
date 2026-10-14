//! blindall-core — domain logic ported from the Laravel implementation.
//!
//! Everything in this crate is pure (no I/O): answer sanitization, fuzzy
//! matching, scoring rules, ELO, levels and performance metrics.
//!
//! Parity contract: functions are bug-for-bug ports of their PHP counterparts.
//! Where PHP behavior is accidental (substring replaces, byte-level
//! levenshtein, the >255-char quirk), it is reproduced and documented.

pub mod elo;
pub mod levels;
pub mod levenshtein;
pub mod matching;
pub mod messages;
pub mod phpser;
pub mod metrics;
pub mod sanitize;
pub mod scoring;
pub mod types;
