use {
    crate::payload::convert,
    gluesql_core::prelude::Glue as CoreGlue,
    gluesql_memory_storage::MemoryStorage,
    napi::{Error, Result, Status},
    napi_derive::napi,
};

#[napi]
pub struct Glue {
    inner: CoreGlue<MemoryStorage>,
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
            inner: CoreGlue::new(MemoryStorage::default()),
        }
    }

    #[napi]
    #[allow(clippy::needless_pass_by_value)]
    pub fn query(&mut self, sql: String) -> Result<String> {
        let payloads = self
            .inner
            .execute(sql)
            .map_err(|error| to_napi_error(&error))?;

        serde_json::to_string(&convert(payloads)).map_err(|error| to_napi_error(&error))
    }
}

#[napi]
#[allow(dead_code)]
pub fn gluesql() -> Glue {
    Glue::new()
}

fn to_napi_error(error: &impl ToString) -> Error {
    Error::new(Status::GenericFailure, error.to_string())
}
