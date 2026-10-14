//! Canonical HTTP routes and messaging topics.
//!
//! The root-package guest builds its HTTP and messaging routers from these
//! tables, and a stem publishes to the same topic constant its consumer
//! subscribes to, so producers and consumers agree on the surface by
//! construction rather than by review.

pub mod http;
pub mod topic;
