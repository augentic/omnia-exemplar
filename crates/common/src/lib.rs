//! # Common
//!
//! What every stem shares: the topic table a publisher and its consumer
//! agree on.
//!
//! The capability bound a stem's `handlers()` takes is the SDK's own
//! `omnia_sdk::Capabilities`, so this crate links nothing from omnia.

pub mod topic;
