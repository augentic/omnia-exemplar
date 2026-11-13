//! # Common
//!
//! What every stem shares: the topic table a publisher and its consumer
//! agree on.
//!
//! Each stem declares the capability bound its `handlers()` takes, a
//! `Requires` trait over the SDK's capability traits, so this crate links
//! nothing from omnia.

pub mod topic;
