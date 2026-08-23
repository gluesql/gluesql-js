#![deny(clippy::str_to_string)]

mod backend;
mod file;

pub use {
    backend::OpfsBackend,
    file::{RandomAccessFile, SyncFile},
    gluesql_redb_storage::RedbStorage,
};

use {
    gluesql_core::error::{Error, Result},
    gluesql_redb_storage::REDB_STORAGE_FORMAT_VERSION,
    redb::{Database, TableDefinition},
};

const STORAGE_META_TABLE: TableDefinition<&str, u32> = TableDefinition::new("__GLUESQL_META__");
const STORAGE_META_VERSION_KEY: &str = "storage_format_version";

pub async fn open(namespace: &str) -> Result<RedbStorage> {
    let file = SyncFile::open(&format!("{namespace}.db")).await?;
    let fresh = file.size()? == 0;

    let backend = OpfsBackend::new(file);
    let database = redb::Builder::new()
        .create_with_file_format_v3(true)
        .create_with_backend(backend)
        .map_err(|error| storage_error(&error))?;

    if fresh {
        initialize_format_version(&database)?;
    }

    RedbStorage::from_database(database)
}

pub(crate) fn initialize_format_version(database: &Database) -> Result<()> {
    let txn = database.begin_write().map_err(|e| storage_error(&e))?;

    {
        let mut table = txn
            .open_table(STORAGE_META_TABLE)
            .map_err(|e| storage_error(&e))?;
        table
            .insert(STORAGE_META_VERSION_KEY, REDB_STORAGE_FORMAT_VERSION)
            .map_err(|e| storage_error(&e))?;
    }

    txn.commit().map_err(|e| storage_error(&e))
}

fn storage_error(error: &dyn std::fmt::Display) -> Error {
    Error::StorageMsg(format!("opfs-storage: {error}"))
}
