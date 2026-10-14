//! Environment-agnostic topic suffixes: one constant per topic, each added at the end.
//!
//! Prefix a suffix with `{env}-` through [`crate::config::topic`] or
//! [`crate::config::topic_for`] before publishing or subscribing.
