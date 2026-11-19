use {
    crate::node::engines::Engine,
    gluesql_core::error::{Error, Result},
    gluesql_memory_storage::MemoryStorage,
    serde::Deserialize,
    serde_json::Value as Json,
};

#[cfg(feature = "redb")]
use gluesql_redb_storage::RedbStorage;

/// Backends compiled into this build, sorted alphabetically.
///
/// Each backend is a cargo feature, so a build only carries what it was asked
/// for. `storages()` is what tells JavaScript which ones are available.
pub fn storages() -> Vec<String> {
    let mut storages = vec!["memory"];

    #[cfg(feature = "redb")]
    storages.push("redb");

    storages.sort_unstable();

    storages.into_iter().map(str::to_owned).collect()
}

/// Storage backend descriptor coming from JavaScript.
///
/// The `storage` field selects the backend and every backend specific option
/// lives in the same object:
///
/// ```javascript
/// db.addEngine('scratch', { storage: 'memory' });
/// ```
///
/// Unknown keys are rejected: a misspelled option would otherwise be dropped
/// silently and hand back a backend configured with defaults. A backend that
/// this build was not compiled with is reported as an unknown `storage` value.
#[derive(Deserialize)]
#[serde(tag = "storage", rename_all = "camelCase", deny_unknown_fields)]
pub enum StorageConfig {
    Memory {},
    #[cfg(feature = "redb")]
    Redb {
        path: String,
    },
}

impl StorageConfig {
    pub fn parse(config: Json) -> Result<Self> {
        serde_json::from_value(config)
            .map_err(|error| Error::StorageMsg(format!("invalid storage config: {error}")))
    }

    // Infallible when the build carries no backend but `memory`.
    #[allow(clippy::unnecessary_wraps)]
    pub fn open(self) -> Result<Box<dyn Engine>> {
        match self {
            Self::Memory {} => Ok(Box::new(MemoryStorage::default())),
            #[cfg(feature = "redb")]
            Self::Redb { path } => RedbStorage::new(path).map(box_storage),
        }
    }
}

#[cfg(feature = "redb")]
fn box_storage<T: Engine + 'static>(storage: T) -> Box<dyn Engine> {
    Box::new(storage)
}
