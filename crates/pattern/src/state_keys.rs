//! State store key conventions.
//!
//! All pattern state lives under the `pattern.` prefix, namespaced by
//! purpose and then by identifier:
//!
//! | Key | Value |
//! | --- | --- |
//! | `pattern.quota.{tenant}` | the tenant's [`QuotaRecord`](crate::cas::QuotaRecord) |
//! | `pattern.segment.{code}` | the decoded [`Segment`](crate::decode::Segment) |
//!
//! An identifier from a request becomes a key segment only when
//! [`is_portable`] accepts it, so every key is portable across state stores.

#[must_use]
pub fn quota(tenant: &str) -> String {
    format!("pattern.quota.{tenant}")
}

#[must_use]
pub fn segment(code: &str) -> String {
    format!("pattern.segment.{code}")
}

// A segment sits between `.` separators, so it never holds one.
#[must_use]
pub fn is_portable(segment: &str) -> bool {
    !segment.is_empty()
        && segment.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}
