//! Environment-agnostic topic suffixes: one constant per topic, each added at the end.
//!
//! A publisher and its consumer name one constant here, so the two agree on
//! the topic by construction. Prefix a suffix with `{env}-` through
//! [`crate::config::topic`] or [`crate::config::topic_for`] before publishing
//! or subscribing.
