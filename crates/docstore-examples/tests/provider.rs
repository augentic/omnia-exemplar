#![allow(missing_docs)]

//! Filter-evaluating in-memory mock of the [`DocumentStore`] capability.
//!
//! Unlike the pass-through mock in `capability-examples`, this mock
//! *evaluates* the query: the [`Filter`] tree is applied to the stored JSON
//! documents, `order_by` sorts the matches, and `limit` plus a numeric
//! continuation token paginate them — so the tests exercise the handlers'
//! filter construction end to end.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use anyhow::{Result, anyhow};
use omnia_guest::DocumentStore;
use omnia_guest::document_store::{
    ComparisonOp, Document, Filter, QueryOptions, QueryResult, ScalarValue,
};
use serde_json::Value;

type Documents = BTreeMap<(String, String), Vec<u8>>;

#[derive(Default, Clone)]
pub struct MockProvider {
    documents: Arc<Mutex<Documents>>,
}

#[allow(clippy::missing_panics_doc)]
impl MockProvider {
    #[must_use]
    pub fn document(&self, store: &str, id: &str) -> Option<Vec<u8>> {
        self.documents.lock().expect("lock").get(&(store.to_string(), id.to_string())).cloned()
    }
}

impl DocumentStore for MockProvider {
    fn get(&self, store: &str, id: &str) -> impl Future<Output = Result<Option<Document>>> {
        let Ok(documents) = self.documents.lock() else {
            return std::future::ready(Err(anyhow!("failed to obtain lock on documents")));
        };
        std::future::ready(Ok(documents.get(&(store.to_string(), id.to_string())).cloned().map(
            |data| Document {
                id: id.to_string(),
                data,
            },
        )))
    }

    fn insert(&self, store: &str, doc: &Document) -> impl Future<Output = Result<()>> {
        let key = (store.to_string(), doc.id.clone());
        let Ok(mut documents) = self.documents.lock() else {
            return std::future::ready(Err(anyhow!("failed to obtain lock on documents")));
        };
        if documents.contains_key(&key) {
            return std::future::ready(Err(anyhow!("document already exists")));
        }
        documents.insert(key, doc.data.clone());
        std::future::ready(Ok(()))
    }

    fn put(&self, store: &str, doc: &Document) -> impl Future<Output = Result<()>> {
        let Ok(mut documents) = self.documents.lock() else {
            return std::future::ready(Err(anyhow!("failed to obtain lock on documents")));
        };
        documents.insert((store.to_string(), doc.id.clone()), doc.data.clone());
        std::future::ready(Ok(()))
    }

    fn delete(&self, store: &str, id: &str) -> impl Future<Output = Result<bool>> {
        let Ok(mut documents) = self.documents.lock() else {
            return std::future::ready(Err(anyhow!("failed to obtain lock on documents")));
        };
        std::future::ready(Ok(documents.remove(&(store.to_string(), id.to_string())).is_some()))
    }

    fn query(
        &self, store: &str, options: QueryOptions,
    ) -> impl Future<Output = Result<QueryResult>> {
        let Ok(documents) = self.documents.lock() else {
            return std::future::ready(Err(anyhow!("failed to obtain lock on documents")));
        };
        std::future::ready(run_query(&documents, store, &options))
    }
}

/// Evaluate the query synchronously over the stored documents.
fn run_query(documents: &Documents, store: &str, options: &QueryOptions) -> Result<QueryResult> {
    let mut matches: Vec<(String, Value, Vec<u8>)> = Vec::new();
    for ((owner, id), data) in documents {
        if owner != store {
            continue;
        }
        let value: Value = serde_json::from_slice(data)?;
        if options.filter.as_ref().is_none_or(|filter| eval(filter, &value)) {
            matches.push((id.clone(), value, data.clone()));
        }
    }

    matches.sort_by(|a, b| {
        for sort in &options.order_by {
            let ordering = cmp_json(a.1.get(&sort.field), b.1.get(&sort.field));
            let ordering = if sort.descending { ordering.reverse() } else { ordering };
            if ordering != Ordering::Equal {
                return ordering;
            }
        }
        Ordering::Equal
    });

    let offset = match &options.continuation {
        None => 0,
        Some(token) => token
            .parse::<usize>()
            .map_err(|_error| anyhow!("invalid continuation token: {token}"))?,
    };
    let total = matches.len();
    let end = options.limit.map_or(total, |limit| {
        let limit = usize::try_from(limit).expect("limit fits in usize");
        offset.saturating_add(limit).min(total)
    });

    let documents = matches
        .get(offset..end)
        .unwrap_or_default()
        .iter()
        .map(|(id, _, data)| Document {
            id: id.clone(),
            data: data.clone(),
        })
        .collect();
    let continuation = (end < total).then(|| end.to_string());

    Ok(QueryResult {
        documents,
        continuation,
    })
}

/// Apply a filter tree to one parsed document.
fn eval(filter: &Filter, doc: &Value) -> bool {
    match filter {
        Filter::Compare { field, op, value } => compare(doc.get(field), *op, value),
        Filter::InList { field, values } => {
            values.iter().any(|value| is_equal(doc.get(field), value))
        }
        Filter::NotInList { field, values } => {
            !values.iter().any(|value| is_equal(doc.get(field), value))
        }
        Filter::IsNull(field) => doc.get(field).is_none_or(Value::is_null),
        Filter::IsNotNull(field) => doc.get(field).is_some_and(|value| !value.is_null()),
        Filter::Contains { field, pattern } => {
            text(doc, field).is_some_and(|value| value.contains(pattern))
        }
        Filter::StartsWith { field, pattern } => {
            text(doc, field).is_some_and(|value| value.starts_with(pattern))
        }
        Filter::EndsWith { field, pattern } => {
            text(doc, field).is_some_and(|value| value.ends_with(pattern))
        }
        Filter::And(filters) => filters.iter().all(|inner| eval(inner, doc)),
        Filter::Or(filters) => filters.iter().any(|inner| eval(inner, doc)),
        Filter::Not(inner) => !eval(inner, doc),
    }
}

fn text<'a>(doc: &'a Value, field: &str) -> Option<&'a str> {
    doc.get(field)?.as_str()
}

/// Order a document field against a filter scalar, when comparable.
fn scalar_cmp(value: &Value, scalar: &ScalarValue) -> Option<Ordering> {
    match (value, scalar) {
        (Value::String(s), ScalarValue::Str(t) | ScalarValue::Timestamp(t)) => {
            Some(s.as_str().cmp(t))
        }
        (Value::Number(n), ScalarValue::Int32(i)) => Some(n.as_f64()?.total_cmp(&f64::from(*i))),
        (Value::Number(n), ScalarValue::Float64(f)) => Some(n.as_f64()?.total_cmp(f)),
        (Value::Bool(b), ScalarValue::Bool(c)) => Some(b.cmp(c)),
        (Value::Null, ScalarValue::Null) => Some(Ordering::Equal),
        _ => None,
    }
}

fn is_equal(value: Option<&Value>, scalar: &ScalarValue) -> bool {
    value.and_then(|inner| scalar_cmp(inner, scalar)) == Some(Ordering::Equal)
}

fn compare(value: Option<&Value>, op: ComparisonOp, scalar: &ScalarValue) -> bool {
    let ordering = value.and_then(|inner| scalar_cmp(inner, scalar));
    match op {
        ComparisonOp::Eq => ordering == Some(Ordering::Equal),
        // Incomparable pairs (missing field, or null vs a string) count as
        // "not equal", matching the default backend's `Ne` semantics.
        ComparisonOp::Ne => ordering != Some(Ordering::Equal),
        ComparisonOp::Gt => ordering == Some(Ordering::Greater),
        ComparisonOp::Gte => matches!(ordering, Some(Ordering::Greater | Ordering::Equal)),
        ComparisonOp::Lt => ordering == Some(Ordering::Less),
        ComparisonOp::Lte => matches!(ordering, Some(Ordering::Less | Ordering::Equal)),
    }
}

/// Order two document field values for sorting.
fn cmp_json(a: Option<&Value>, b: Option<&Value>) -> Ordering {
    match (a, b) {
        (Some(Value::String(x)), Some(Value::String(y))) => x.cmp(y),
        (Some(Value::Number(x)), Some(Value::Number(y))) => {
            x.as_f64().unwrap_or(f64::NAN).total_cmp(&y.as_f64().unwrap_or(f64::NAN))
        }
        _ => Ordering::Equal,
    }
}
