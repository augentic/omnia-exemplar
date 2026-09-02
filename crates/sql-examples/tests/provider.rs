#![allow(missing_docs)]

//! Spy `TableStore` mock for the SQL-example handlers.
//!
//! The mock keeps in-memory agency and feed tables and *recognizes* the
//! ORM-generated SQL: it parses the double-quoted column lists out of the
//! rendered statements, checks the bound parameters against them, and
//! answers the JOIN select with aliased columns — erroring loudly on any
//! statement shape it does not expect. Every statement is also recorded so
//! tests can assert on what the handlers sent.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use anyhow::{Context as _, Result, anyhow, bail, ensure};
use omnia_guest::TableStore;
use omnia_guest::orm::{DataType, Field, Row};

/// One agency row as stored by the mock, keyed by `agency_id`.
#[derive(Debug, Clone)]
pub struct AgencyRow {
    pub name: String,
    pub url: Option<String>,
    pub timezone: Option<String>,
    pub created_at: String,
}

/// One feed row as stored by the mock, keyed by `feed_id`.
#[derive(Debug, Clone)]
pub struct FeedRow {
    pub agency_id: i64,
    pub description: String,
    pub created_at: String,
}

#[derive(Default, Clone)]
pub struct MockProvider {
    agencies: Arc<Mutex<BTreeMap<i64, AgencyRow>>>,
    feeds: Arc<Mutex<BTreeMap<i64, FeedRow>>>,
    statements: Arc<Mutex<Vec<String>>>,
}

#[allow(clippy::missing_panics_doc)]
#[allow(dead_code)] // Not every test binary uses every accessor.
impl MockProvider {
    #[must_use]
    pub fn agency(&self, id: i64) -> Option<AgencyRow> {
        self.agencies.lock().expect("lock").get(&id).cloned()
    }

    #[must_use]
    pub fn feed(&self, id: i64) -> Option<FeedRow> {
        self.feeds.lock().expect("lock").get(&id).cloned()
    }

    /// Every SQL statement the handlers sent, in order.
    #[must_use]
    pub fn statements(&self) -> Vec<String> {
        self.statements.lock().expect("lock").clone()
    }

    fn record(&self, sql: &str) -> Result<()> {
        self.statements
            .lock()
            .map_err(|_error| anyhow!("failed to obtain lock on statements"))?
            .push(sql.to_string());
        Ok(())
    }

    /// The test tables are tiny, so the query paths work on owned snapshots
    /// and the lock guards drop immediately.
    fn agencies_snapshot(&self) -> Result<BTreeMap<i64, AgencyRow>> {
        Ok(self
            .agencies
            .lock()
            .map_err(|_error| anyhow!("failed to obtain lock on agencies"))?
            .clone())
    }

    fn feeds_snapshot(&self) -> Result<BTreeMap<i64, FeedRow>> {
        Ok(self.feeds.lock().map_err(|_error| anyhow!("failed to obtain lock on feeds"))?.clone())
    }

    fn exec_sync(&self, sql: &str, params: &[DataType]) -> Result<u32> {
        self.record(sql)?;

        if sql.starts_with("CREATE TABLE IF NOT EXISTS") {
            return Ok(0);
        }
        if sql.starts_with("INSERT INTO \"agency\"") {
            let values = insert_values(sql, params)?;
            let id = int_value(&values, "agency_id")?;
            let row = AgencyRow {
                name: str_value(&values, "name")?,
                url: opt_str_value(&values, "url")?,
                timezone: opt_str_value(&values, "timezone")?,
                created_at: str_value(&values, "created_at")?,
            };
            {
                let mut agencies = self
                    .agencies
                    .lock()
                    .map_err(|_error| anyhow!("failed to obtain lock on agencies"))?;
                ensure!(!agencies.contains_key(&id), "agency {id} violates the primary key");
                agencies.insert(id, row)
            };
            return Ok(1);
        }
        if sql.starts_with("INSERT INTO \"feed\"") {
            let values = insert_values(sql, params)?;
            let id = int_value(&values, "feed_id")?;
            let row = FeedRow {
                agency_id: int_value(&values, "agency_id")?,
                description: str_value(&values, "description")?,
                created_at: str_value(&values, "created_at")?,
            };
            {
                let mut feeds = self
                    .feeds
                    .lock()
                    .map_err(|_error| anyhow!("failed to obtain lock on feeds"))?;
                ensure!(!feeds.contains_key(&id), "feed {id} violates the primary key");
                feeds.insert(id, row)
            };
            return Ok(1);
        }
        if sql.starts_with("UPDATE \"agency\" SET ") {
            let (set_fragment, where_fragment) =
                sql.split_once(" WHERE ").context("UPDATE without a WHERE clause")?;
            let expected_filter = format!("(\"agency\".\"agency_id\") = (${})", params.len());
            ensure!(
                where_fragment == expected_filter,
                "unexpected UPDATE filter: {where_fragment}"
            );
            let set_columns = quoted_idents(
                set_fragment.strip_prefix("UPDATE \"agency\" SET ").unwrap_or_default(),
            );
            ensure!(
                set_columns.len() + 1 == params.len(),
                "SET columns and bound parameters disagree: {sql}"
            );
            let id = int_param(params.last())?;
            let affected = {
                let mut agencies = self
                    .agencies
                    .lock()
                    .map_err(|_error| anyhow!("failed to obtain lock on agencies"))?;
                match agencies.get_mut(&id) {
                    None => 0,
                    Some(row) => {
                        for (column, param) in set_columns.iter().zip(params) {
                            match column.as_str() {
                                "name" => row.name = str_param(Some(param))?,
                                "url" => row.url = opt_str_param(Some(param))?,
                                "timezone" => row.timezone = opt_str_param(Some(param))?,
                                other => bail!("unexpected SET column: {other}"),
                            }
                        }
                        1
                    }
                }
            };
            return Ok(affected);
        }
        if sql.starts_with("DELETE FROM \"feed\"") {
            ensure!(
                sql.contains("(\"feed\".\"feed_id\") = ($1)"),
                "unexpected DELETE filter: {sql}"
            );
            let id = int_param(params.first())?;
            let removed = self
                .feeds
                .lock()
                .map_err(|_error| anyhow!("failed to obtain lock on feeds"))?
                .remove(&id)
                .is_some();
            return Ok(u32::from(removed));
        }

        bail!("unexpected statement: {sql}")
    }

    fn query_sync(&self, sql: &str, params: &[DataType]) -> Result<Vec<Row>> {
        self.record(sql)?;
        ensure!(sql.starts_with("SELECT"), "unexpected query: {sql}");

        if sql.contains("LEFT JOIN \"agency\"") {
            return self.joined_feeds(sql, params);
        }
        if sql.contains("FROM \"agency\"") {
            return self.agency_select(sql, params);
        }
        if sql.contains("FROM \"feed\"") {
            return self.feed_select(sql, params);
        }
        bail!("unexpected query: {sql}")
    }

    /// Answer the plain agency selects: fetch by id, the max-id probe, and
    /// the newest-first listing.
    fn agency_select(&self, sql: &str, params: &[DataType]) -> Result<Vec<Row>> {
        let agencies = self.agencies_snapshot()?;

        if sql.contains(" WHERE ") {
            ensure!(
                sql.contains("(\"agency\".\"agency_id\") = ($1)"),
                "unexpected agency filter: {sql}"
            );
            let id = int_param(params.first())?;
            return Ok(agencies.get(&id).map(|row| agency_row(0, id, row)).into_iter().collect());
        }
        if sql.contains("ORDER BY \"agency\".\"agency_id\" DESC") {
            // The max-id probe: newest id, LIMIT bound as a parameter.
            ensure!(limit_param(params)? == Some(1), "the max-id probe should have LIMIT 1");
            return Ok(agencies
                .last_key_value()
                .map(|(id, row)| agency_row(0, *id, row))
                .into_iter()
                .collect());
        }
        if sql.contains("ORDER BY \"agency\".\"created_at\" DESC") {
            let limit = limit_param(params)?.unwrap_or(usize::MAX);
            let mut rows: Vec<(&i64, &AgencyRow)> = agencies.iter().collect();
            // Newest first; ids break creation-time ties deterministically.
            rows.sort_by(|a, b| (&b.1.created_at, b.0).cmp(&(&a.1.created_at, a.0)));
            return Ok(rows
                .into_iter()
                .take(limit)
                .enumerate()
                .map(|(index, (id, row))| agency_row(index, *id, row))
                .collect());
        }
        bail!("unexpected agency query: {sql}")
    }

    /// Answer the plain feed selects: per-agency listing and the max-id probe.
    fn feed_select(&self, sql: &str, params: &[DataType]) -> Result<Vec<Row>> {
        let feeds = self.feeds_snapshot()?;

        if sql.contains(" WHERE ") {
            ensure!(
                sql.contains("(\"feed\".\"agency_id\") = ($1)"),
                "unexpected feed filter: {sql}"
            );
            let agency_id = int_param(params.first())?;
            let mut rows: Vec<(&i64, &FeedRow)> =
                feeds.iter().filter(|(_, row)| row.agency_id == agency_id).collect();
            rows.sort_by(|a, b| (&b.1.created_at, b.0).cmp(&(&a.1.created_at, a.0)));
            return Ok(rows
                .into_iter()
                .enumerate()
                .map(|(index, (id, row))| feed_row(index, *id, row))
                .collect());
        }
        if sql.contains("ORDER BY \"feed\".\"feed_id\" DESC") {
            ensure!(limit_param(params)? == Some(1), "the max-id probe should have LIMIT 1");
            return Ok(feeds
                .last_key_value()
                .map(|(id, row)| feed_row(0, *id, row))
                .into_iter()
                .collect());
        }
        bail!("unexpected feed query: {sql}")
    }

    /// Answer the `FeedWithAgency` JOIN select with aliased agency columns.
    fn joined_feeds(&self, sql: &str, params: &[DataType]) -> Result<Vec<Row>> {
        ensure!(
            sql.contains(
                "LEFT JOIN \"agency\" ON (\"feed\".\"agency_id\") = (\"agency\".\"agency_id\")"
            ),
            "unexpected join condition: {sql}"
        );
        ensure!(
            sql.contains("ORDER BY \"feed\".\"created_at\" DESC"),
            "the joined listing should sort newest first: {sql}"
        );
        let limit = limit_param(params)?.unwrap_or(usize::MAX);

        let agencies = self.agencies_snapshot()?;
        let feeds = self.feeds_snapshot()?;
        let mut rows: Vec<(&i64, &FeedRow)> = feeds.iter().collect();
        rows.sort_by(|a, b| (&b.1.created_at, b.0).cmp(&(&a.1.created_at, a.0)));

        rows.into_iter()
            .take(limit)
            .enumerate()
            .map(|(index, (id, feed))| {
                let agency = agencies
                    .get(&feed.agency_id)
                    .with_context(|| format!("feed {id} joins a missing agency"))?;
                let mut row = feed_row(index, *id, feed);
                row.fields.extend([
                    field_str("agency_name", &agency.name),
                    field_opt_str("agency_url", agency.url.as_deref()),
                    field_opt_str("agency_timezone", agency.timezone.as_deref()),
                ]);
                Ok(row)
            })
            .collect()
    }
}

impl TableStore for MockProvider {
    fn query(
        &self, _conn: String, query: String, params: Vec<DataType>,
    ) -> impl Future<Output = Result<Vec<Row>>> {
        std::future::ready(self.query_sync(&query, &params))
    }

    fn exec(
        &self, _conn: String, query: String, params: Vec<DataType>,
    ) -> impl Future<Output = Result<u32>> {
        std::future::ready(self.exec_sync(&query, &params))
    }
}

/// The identifiers double-quoted inside a fragment of rendered SQL.
fn quoted_idents(fragment: &str) -> Vec<String> {
    fragment.split('"').skip(1).step_by(2).map(ToString::to_string).collect()
}

/// Zip an INSERT's column list with its bound parameters.
fn insert_values<'a>(sql: &str, params: &'a [DataType]) -> Result<BTreeMap<String, &'a DataType>> {
    let columns_fragment = sql
        .split_once('(')
        .and_then(|(_, rest)| rest.split_once(") VALUES"))
        .map(|(columns, _)| columns)
        .with_context(|| format!("unexpected INSERT shape: {sql}"))?;
    let columns = quoted_idents(columns_fragment);
    ensure!(columns.len() == params.len(), "INSERT columns and bound parameters disagree: {sql}");
    Ok(columns.into_iter().zip(params).collect())
}

fn int_value(values: &BTreeMap<String, &DataType>, column: &str) -> Result<i64> {
    int_param(values.get(column).copied())
}

fn str_value(values: &BTreeMap<String, &DataType>, column: &str) -> Result<String> {
    str_param(values.get(column).copied())
}

fn opt_str_value(values: &BTreeMap<String, &DataType>, column: &str) -> Result<Option<String>> {
    opt_str_param(values.get(column).copied())
}

fn int_param(param: Option<&DataType>) -> Result<i64> {
    match param {
        Some(DataType::Int64(Some(value))) => Ok(*value),
        other => bail!("expected an Int64 parameter, got {other:?}"),
    }
}

fn str_param(param: Option<&DataType>) -> Result<String> {
    match param {
        Some(DataType::Str(Some(value))) => Ok(value.clone()),
        other => bail!("expected a Str parameter, got {other:?}"),
    }
}

fn opt_str_param(param: Option<&DataType>) -> Result<Option<String>> {
    match param {
        Some(DataType::Str(value)) => Ok(value.clone()),
        other => bail!("expected a nullable Str parameter, got {other:?}"),
    }
}

/// The trailing LIMIT bound parameter, when present.
fn limit_param(params: &[DataType]) -> Result<Option<usize>> {
    match params {
        [] => Ok(None),
        [DataType::Uint64(Some(limit))] => {
            Ok(Some(usize::try_from(*limit).context("limit fits in usize")?))
        }
        other => bail!("expected at most a Uint64 LIMIT parameter, got {other:?}"),
    }
}

fn field_int(name: &str, value: i64) -> Field {
    Field {
        name: name.to_string(),
        value: DataType::Int64(Some(value)),
    }
}

fn field_str(name: &str, value: &str) -> Field {
    Field {
        name: name.to_string(),
        value: DataType::Str(Some(value.to_string())),
    }
}

fn field_opt_str(name: &str, value: Option<&str>) -> Field {
    Field {
        name: name.to_string(),
        value: DataType::Str(value.map(ToString::to_string)),
    }
}

fn agency_row(index: usize, id: i64, row: &AgencyRow) -> Row {
    Row {
        index: index.to_string(),
        fields: vec![
            field_int("agency_id", id),
            field_str("name", &row.name),
            field_opt_str("url", row.url.as_deref()),
            field_opt_str("timezone", row.timezone.as_deref()),
            field_str("created_at", &row.created_at),
        ],
    }
}

fn feed_row(index: usize, id: i64, row: &FeedRow) -> Row {
    Row {
        index: index.to_string(),
        fields: vec![
            field_int("feed_id", id),
            field_int("agency_id", row.agency_id),
            field_str("description", &row.description),
            field_str("created_at", &row.created_at),
        ],
    }
}
