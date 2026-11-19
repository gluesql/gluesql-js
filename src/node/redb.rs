use {
    super::{StorageConfig, execute_query, to_napi_error},
    gluesql_core::{prelude::Glue as CoreGlue, store::Transaction},
    gluesql_redb_storage::RedbStorage,
    napi::{Error, Result},
    napi_derive::napi,
    serde_json::Value as Json,
};

#[napi]
pub struct DirectRedb {
    inner: Option<CoreGlue<RedbStorage>>,
}

#[napi]
impl DirectRedb {
    #[napi]
    #[allow(clippy::needless_pass_by_value)]
    pub fn query(&mut self, sql: String) -> Result<String> {
        let db = self
            .inner
            .as_mut()
            .ok_or_else(|| Error::from_reason("redb database is closed"))?;
        execute_query(db, &sql)
    }

    #[napi]
    pub fn close(&mut self) -> Result<()> {
        if let Some(mut db) = self.inner.take() {
            db.storage.rollback().map_err(to_napi_error)?;
        }
        Ok(())
    }
}

#[napi(ts_args_type = "config: { storage: 'redb'; path: string }")]
#[allow(dead_code)]
pub fn open_redb(config: Json) -> Result<DirectRedb> {
    let StorageConfig::Redb { path } = StorageConfig::parse(config).map_err(to_napi_error)? else {
        return Err(Error::from_reason("direct storage must be redb"));
    };
    let storage = RedbStorage::new(path).map_err(to_napi_error)?;

    Ok(DirectRedb {
        inner: Some(CoreGlue::new(storage)),
    })
}
