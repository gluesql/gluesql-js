mod engines;
mod storage;

use {
    crate::payload::convert,
    engines::Engines,
    gluesql_core::prelude::Glue as CoreGlue,
    napi::{Error, Result},
    napi_derive::napi,
    serde_json::Value as Json,
    storage::StorageConfig,
};

#[napi]
pub struct Glue {
    inner: CoreGlue<Engines>,
}

impl Default for Glue {
    fn default() -> Self {
        Self::new()
    }
}

#[napi]
impl Glue {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self {
            inner: CoreGlue::new(Engines::default()),
        }
    }

    /// Registers a storage backend under `engine`, which is then usable as the
    /// `ENGINE` clause value of `CREATE TABLE`.
    #[napi(ts_args_type = "engine: string, config: Record<string, unknown>")]
    #[allow(clippy::needless_pass_by_value)]
    pub fn add_engine(&mut self, engine: String, config: Json) -> Result<()> {
        let config = StorageConfig::parse(config).map_err(to_napi_error)?;

        self.inner
            .storage
            .add(engine, config)
            .map_err(to_napi_error)
    }

    /// Unregisters a storage backend. Tables of the removed engine become
    /// unreachable, but their data is left untouched.
    #[napi]
    #[allow(clippy::needless_pass_by_value)]
    pub fn remove_engine(&mut self, engine: String) -> Result<()> {
        self.inner.storage.remove(&engine).map_err(to_napi_error)
    }

    /// Registered engine names, sorted alphabetically.
    #[napi]
    pub fn list_engines(&self) -> Vec<String> {
        self.inner.storage.names()
    }

    /// Engine used when `CREATE TABLE` omits the `ENGINE` clause.
    #[napi]
    pub fn default_engine(&self) -> Option<String> {
        self.inner.storage.default_engine()
    }

    #[napi]
    #[allow(clippy::needless_pass_by_value)]
    pub fn set_default_engine(&mut self, engine: String) -> Result<()> {
        self.inner
            .storage
            .set_default(engine)
            .map_err(to_napi_error)
    }

    #[napi]
    #[allow(clippy::needless_pass_by_value)]
    pub fn query(&mut self, sql: String) -> Result<String> {
        let payloads = self.inner.execute(sql).map_err(to_napi_error)?;

        serde_json::to_string(&convert(payloads)).map_err(to_napi_error)
    }
}

#[napi]
#[allow(dead_code)]
pub fn gluesql() -> Glue {
    Glue::new()
}

/// Storage backends this build was compiled with, sorted alphabetically.
#[napi]
#[allow(dead_code)]
pub fn storages() -> Vec<String> {
    storage::storages()
}

#[allow(clippy::needless_pass_by_value)]
fn to_napi_error(error: impl ToString) -> Error {
    Error::from_reason(error.to_string())
}
