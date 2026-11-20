//! # Common
//!
//! What every stem shares: the topic table a publisher and its consumer
//! agree on.
//!
//! A stem reaches the host through `omnia-sdk`'s bindings, so this crate
//! links nothing from omnia.

pub mod topic;
