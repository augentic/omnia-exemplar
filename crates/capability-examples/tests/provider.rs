#![allow(missing_docs)]

//! In-memory mock provider implementing the native side of every capability
//! the example handlers require.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use anyhow::{Result, anyhow};
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

    fn get_range(
        &self, container: &str, name: &str, start: u64, end: u64,
    ) -> impl Future<Output = Result<Vec<u8>>> {
        let Ok(containers) = self.containers.lock() else {
            return std::future::ready(Err(anyhow!("failed to obtain lock on containers")));
        };
        let Some(data) = containers.get(container).and_then(|objects| objects.get(name)) else {
            return std::future::ready(Err(anyhow!("object does not exist")));
        };
        let start = match usize::try_from(start) {
            Ok(start) => start,
            Err(error) => return std::future::ready(Err(error.into())),
        };
        let end = match usize::try_from(end) {
            Ok(end) => end.min(data.len().saturating_sub(1)),
            Err(error) => return std::future::ready(Err(error.into())),
        };
        std::future::ready(Ok(data.get(start..=end).unwrap_or_default().to_vec()))
    }

    fn object_info(
        &self, container: &str, name: &str,
    ) -> impl Future<Output = Result<ObjectMetadata>> {
        let Ok(containers) = self.containers.lock() else {
            return std::future::ready(Err(anyhow!("failed to obtain lock on containers")));
        };
        let Some(data) = containers.get(container).and_then(|objects| objects.get(name)) else {
            return std::future::ready(Err(anyhow!("object does not exist")));
        };
        std::future::ready(Ok(ObjectMetadata {
            name: name.to_string(),
            container: container.to_string(),
            created_at: 0,
            size: data.len() as u64,
        }))
    }

    fn delete_objects(
        &self, container: &str, names: &[String],
    ) -> impl Future<Output = Result<()>> {
        let Ok(mut containers) = self.containers.lock() else {
            return std::future::ready(Err(anyhow!("failed to obtain lock on containers")));
        };
        let Some(objects) = containers.get_mut(container) else {
            return std::future::ready(Err(anyhow!("container does not exist")));
        };
        for name in names {
            objects.remove(name);
        }
        std::future::ready(Ok(()))
    }

    fn clear(&self, container: &str) -> impl Future<Output = Result<()>> {
        let Ok(mut containers) = self.containers.lock() else {
            return std::future::ready(Err(anyhow!("failed to obtain lock on containers")));
        };
        let Some(objects) = containers.get_mut(container) else {
            return std::future::ready(Err(anyhow!("container does not exist")));
        };
        objects.clear();
        std::future::ready(Ok(()))
    }

    fn create_container(&self, name: &str) -> impl Future<Output = Result<()>> {
        let Ok(mut containers) = self.containers.lock() else {
            return std::future::ready(Err(anyhow!("failed to obtain lock on containers")));
        };
        containers.entry(name.to_string()).or_default();
        std::future::ready(Ok(()))
    }

    fn delete_container(&self, name: &str) -> impl Future<Output = Result<()>> {
        let Ok(mut containers) = self.containers.lock() else {
            return std::future::ready(Err(anyhow!("failed to obtain lock on containers")));
        };
        containers.remove(name);
        std::future::ready(Ok(()))
    }

    fn container_exists(&self, name: &str) -> impl Future<Output = Result<bool>> {
        let Ok(containers) = self.containers.lock() else {
            return std::future::ready(Err(anyhow!("failed to obtain lock on containers")));
        };
        std::future::ready(Ok(containers.contains_key(name)))
    }

    fn container_info(&self, container: &str) -> impl Future<Output = Result<ContainerMetadata>> {
        let Ok(containers) = self.containers.lock() else {
            return std::future::ready(Err(anyhow!("failed to obtain lock on containers")));
        };
        if !containers.contains_key(container) {
            return std::future::ready(Err(anyhow!("container does not exist")));
        }
        std::future::ready(Ok(ContainerMetadata {
            name: container.to_string(),
            created_at: 0,
        }))
    }

    fn copy_object(
        &self, src_container: &str, src_name: &str, dest_container: &str, dest_name: &str,
    ) -> impl Future<Output = Result<()>> {
        let Ok(mut containers) = self.containers.lock() else {
            return std::future::ready(Err(anyhow!("failed to obtain lock on containers")));
        };
        let Some(data) =
            containers.get(src_container).and_then(|objects| objects.get(src_name)).cloned()
        else {
            return std::future::ready(Err(anyhow!("object does not exist")));
        };
        let Some(dest_objects) = containers.get_mut(dest_container) else {
            return std::future::ready(Err(anyhow!("destination container does not exist")));
        };
        dest_objects.insert(dest_name.to_string(), data);
        std::future::ready(Ok(()))
    }

    async fn move_object(
        &self, src_container: &str, src_name: &str, dest_container: &str, dest_name: &str,
    ) -> Result<()> {
        BlobStore::copy_object(self, src_container, src_name, dest_container, dest_name).await?;
        BlobStore::delete(self, src_container, src_name).await
    }
}

impl Broadcast for MockProvider {
    fn send(
        &self, name: &str, data: &[u8], sockets: Option<Vec<String>>,
    ) -> impl Future<Output = Result<()>> {
        let Ok(mut broadcasts) = self.broadcasts.lock() else {
            return std::future::ready(Err(anyhow!("failed to obtain lock on broadcasts")));
        };
        broadcasts.push((name.to_string(), data.to_vec(), sockets));
        std::future::ready(Ok(()))
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
        &self, store: &str, _options: QueryOptions,
    ) -> impl Future<Output = Result<QueryResult>> {
        let Ok(documents) = self.documents.lock() else {
            return std::future::ready(Err(anyhow!("failed to obtain lock on documents")));
        };
        let documents = documents
            .iter()
            .filter(|((owner, _), _)| owner == store)
            .map(|((_, id), data)| Document {
                id: id.clone(),
                data: data.clone(),
            })
            .collect();
        std::future::ready(Ok(QueryResult {
            documents,
            continuation: None,
        }))
    }
}

impl TableStore for MockProvider {
    fn query(
        &self, _conn: String, _query: String, params: Vec<DataType>,
    ) -> impl Future<Output = Result<Vec<Row>>> {
        let Some(first) = params.first() else {
            return std::future::ready(Err(anyhow!("missing sensor param")));
        };
        let DataType::Str(Some(sensor)) = first else {
            return std::future::ready(Err(anyhow!("expected string sensor param")));
        };
        let Ok(readings) = self.readings.lock() else {
            return std::future::ready(Err(anyhow!("failed to obtain lock on readings")));
        };
        std::future::ready(Ok(readings
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
            .collect()))
    }

    fn exec(
        &self, _conn: String, _query: String, params: Vec<DataType>,
    ) -> impl Future<Output = Result<u32>> {
        let Some(first) = params.first() else {
            return std::future::ready(Err(anyhow!("missing sensor param")));
        };
        let DataType::Str(Some(sensor)) = first else {
            return std::future::ready(Err(anyhow!("expected string sensor param")));
        };
        let Some(second) = params.get(1) else {
            return std::future::ready(Err(anyhow!("missing value param")));
        };
        let DataType::Double(Some(value)) = second else {
            return std::future::ready(Err(anyhow!("expected double value param")));
        };
        let Ok(mut readings) = self.readings.lock() else {
            return std::future::ready(Err(anyhow!("failed to obtain lock on readings")));
        };
        readings.push((sensor.clone(), *value));
        std::future::ready(Ok(1))
    }
}
