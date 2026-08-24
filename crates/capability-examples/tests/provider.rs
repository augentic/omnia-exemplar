#![allow(missing_docs)]

//! In-memory mock provider implementing the native side of every capability
//! the example operations require.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use anyhow::{Context as _, Result, anyhow};
use omnia_guest::document_store::{Document, QueryOptions, QueryResult};
use omnia_guest::{
    BlobStore, Broadcast, ContainerMetadata, DocumentStore, ObjectMetadata, TableStore,
};
use omnia_wasi_sql::{DataType, Field, Row};

type Containers = BTreeMap<String, BTreeMap<String, Vec<u8>>>;
type Broadcasts = Vec<(String, Vec<u8>, Option<Vec<String>>)>;
type Documents = BTreeMap<(String, String), Vec<u8>>;
type Readings = Vec<(String, f64)>;

#[derive(Default, Clone)]
pub struct MockProvider {
    containers: Arc<Mutex<Containers>>,
    broadcasts: Arc<Mutex<Broadcasts>>,
    documents: Arc<Mutex<Documents>>,
    readings: Arc<Mutex<Readings>>,
}

#[allow(clippy::missing_panics_doc)]
impl MockProvider {
    #[must_use]
    pub fn object(&self, container: &str, name: &str) -> Option<Vec<u8>> {
        self.containers.lock().expect("lock").get(container)?.get(name).cloned()
    }

    #[must_use]
    pub fn broadcasts(&self) -> Broadcasts {
        self.broadcasts.lock().expect("lock").clone()
    }

    #[must_use]
    pub fn document(&self, store: &str, id: &str) -> Option<Vec<u8>> {
        self.documents.lock().expect("lock").get(&(store.to_string(), id.to_string())).cloned()
    }

    #[must_use]
    pub fn readings(&self) -> Readings {
        self.readings.lock().expect("lock").clone()
    }
}

impl BlobStore for MockProvider {
    fn get(&self, container: &str, name: &str) -> impl Future<Output = Result<Option<Vec<u8>>>> {
        std::future::ready(Ok(self.object(container, name)))
    }

    fn put(&self, container: &str, name: &str, data: &[u8]) -> impl Future<Output = Result<()>> {
        let Ok(mut containers) = self.containers.lock() else {
            return std::future::ready(Err(anyhow!("failed to obtain lock on containers")));
        };
        let Some(objects) = containers.get_mut(container) else {
            return std::future::ready(Err(anyhow!("container does not exist")));
        };
        objects.insert(name.to_string(), data.to_vec());
        std::future::ready(Ok(()))
    }

    fn delete(&self, container: &str, name: &str) -> impl Future<Output = Result<()>> {
        let Ok(mut containers) = self.containers.lock() else {
            return std::future::ready(Err(anyhow!("failed to obtain lock on containers")));
        };
        let Some(objects) = containers.get_mut(container) else {
            return std::future::ready(Err(anyhow!("container does not exist")));
        };
        objects.remove(name);
        std::future::ready(Ok(()))
    }

    fn has(&self, container: &str, name: &str) -> impl Future<Output = Result<bool>> {
        std::future::ready(Ok(self.object(container, name).is_some()))
    }

    fn list(&self, container: &str) -> impl Future<Output = Result<Vec<String>>> {
        let Ok(containers) = self.containers.lock() else {
            return std::future::ready(Err(anyhow!("failed to obtain lock on containers")));
        };
        let Some(objects) = containers.get(container) else {
            return std::future::ready(Err(anyhow!("container does not exist")));
        };
        std::future::ready(Ok(objects.keys().cloned().collect()))
    }

    async fn get_range(
        &self, container: &str, name: &str, start: u64, end: u64,
    ) -> Result<Vec<u8>> {
        let data = self.object(container, name).context("object does not exist")?;
        let start = usize::try_from(start)?;
        let end = usize::try_from(end)?.min(data.len().saturating_sub(1));
        Ok(data.get(start..=end).unwrap_or_default().to_vec())
    }

    async fn object_info(&self, container: &str, name: &str) -> Result<ObjectMetadata> {
        let data = self.object(container, name).context("object does not exist")?;
        Ok(ObjectMetadata {
            name: name.to_string(),
            container: container.to_string(),
            created_at: 0,
            size: data.len() as u64,
        })
    }

    async fn delete_objects(&self, container: &str, names: &[String]) -> Result<()> {
        let mut containers = self.containers.lock().expect("lock");
        let objects = containers.get_mut(container).context("container does not exist")?;
        for name in names {
            objects.remove(name);
        }
        drop(containers);
        Ok(())
    }

    async fn clear(&self, container: &str) -> Result<()> {
        self.containers
            .lock()
            .expect("lock")
            .get_mut(container)
            .context("container does not exist")?
            .clear();
        Ok(())
    }

    async fn create_container(&self, name: &str) -> Result<()> {
        self.containers.lock().expect("lock").entry(name.to_string()).or_default();
        Ok(())
    }

    async fn delete_container(&self, name: &str) -> Result<()> {
        self.containers.lock().expect("lock").remove(name);
        Ok(())
    }

    async fn container_exists(&self, name: &str) -> Result<bool> {
        Ok(self.containers.lock().expect("lock").contains_key(name))
    }

    async fn container_info(&self, container: &str) -> Result<ContainerMetadata> {
        if !self.containers.lock().expect("lock").contains_key(container) {
            return Err(anyhow!("container does not exist"));
        }
        Ok(ContainerMetadata {
            name: container.to_string(),
            created_at: 0,
        })
    }

    async fn copy_object(
        &self, src_container: &str, src_name: &str, dest_container: &str, dest_name: &str,
    ) -> Result<()> {
        let data = self.object(src_container, src_name).context("object does not exist")?;
        self.containers
            .lock()
            .expect("lock")
            .get_mut(dest_container)
            .context("destination container does not exist")?
            .insert(dest_name.to_string(), data);
        Ok(())
    }

    async fn move_object(
        &self, src_container: &str, src_name: &str, dest_container: &str, dest_name: &str,
    ) -> Result<()> {
        BlobStore::copy_object(self, src_container, src_name, dest_container, dest_name).await?;
        BlobStore::delete(self, src_container, src_name).await
    }
}

impl Broadcast for MockProvider {
    async fn send(&self, name: &str, data: &[u8], sockets: Option<Vec<String>>) -> Result<()> {
        self.broadcasts.lock().expect("lock").push((name.to_string(), data.to_vec(), sockets));
        Ok(())
    }
}

impl DocumentStore for MockProvider {
    async fn get(&self, store: &str, id: &str) -> Result<Option<Document>> {
        Ok(self.document(store, id).map(|data| Document {
            id: id.to_string(),
            data,
        }))
    }

    async fn insert(&self, store: &str, doc: &Document) -> Result<()> {
        let key = (store.to_string(), doc.id.clone());
        let mut documents = self.documents.lock().expect("lock");
        if documents.contains_key(&key) {
            return Err(anyhow!("document already exists"));
        }
        documents.insert(key, doc.data.clone());
        drop(documents);
        Ok(())
    }

    async fn put(&self, store: &str, doc: &Document) -> Result<()> {
        self.documents
            .lock()
            .expect("lock")
            .insert((store.to_string(), doc.id.clone()), doc.data.clone());
        Ok(())
    }

    async fn delete(&self, store: &str, id: &str) -> Result<bool> {
        Ok(self
            .documents
            .lock()
            .expect("lock")
            .remove(&(store.to_string(), id.to_string()))
            .is_some())
    }

    async fn query(&self, store: &str, _options: QueryOptions) -> Result<QueryResult> {
        let documents = self
            .documents
            .lock()
            .expect("lock")
            .iter()
            .filter(|((owner, _), _)| owner == store)
            .map(|((_, id), data)| Document {
                id: id.clone(),
                data: data.clone(),
            })
            .collect();
        Ok(QueryResult {
            documents,
            continuation: None,
        })
    }
}

impl TableStore for MockProvider {
    async fn query(
        &self, _conn: String, _query: String, params: Vec<DataType>,
    ) -> Result<Vec<Row>> {
        let DataType::Str(Some(sensor)) = params.first().context("missing sensor param")? else {
            return Err(anyhow!("expected string sensor param"));
        };
        Ok(self
            .readings
            .lock()
            .expect("lock")
            .iter()
            .filter(|(name, _)| name == sensor)
            .enumerate()
            .map(|(index, (name, value))| Row {
                index: index.to_string(),
                fields: vec![
                    Field {
                        name: "sensor".to_string(),
                        value: DataType::Str(Some(name.clone())),
                    },
                    Field {
                        name: "value".to_string(),
                        value: DataType::Double(Some(*value)),
                    },
                ],
            })
            .collect())
    }

    async fn exec(&self, _conn: String, _query: String, params: Vec<DataType>) -> Result<u32> {
        let DataType::Str(Some(sensor)) = params.first().context("missing sensor param")? else {
            return Err(anyhow!("expected string sensor param"));
        };
        let DataType::Double(Some(value)) = params.get(1).context("missing value param")? else {
            return Err(anyhow!("expected double value param"));
        };
        self.readings.lock().expect("lock").push((sensor.clone(), *value));
        Ok(1)
    }
}
