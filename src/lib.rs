mod convert;
mod memory_storage;
mod utils;
mod web_storage;

use wasm_bindgen::prelude::*;

use convert::convert;

pub use memory_storage::MemoryStorage;
pub use web_storage::{LocalStorage, SessionStorage};

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

enum Storage {
    Memory(MemoryStorage),
    Local(LocalStorage),
    Session(SessionStorage),
}

#[wasm_bindgen]
pub struct Glue {
    storage: Option<Storage>,
}

#[wasm_bindgen]
impl Glue {
    #[wasm_bindgen(constructor)]
    pub fn new(storage_type: &str, namespace: &JsValue) -> Result<Glue, JsValue> {
        utils::set_panic_hook();

        log(&format!("[GlueSQL] storage: {}", storage_type));

        let get_namespace = || match namespace.as_string() {
            Some(namespace) => {
                log(&format!("[GlueSQL] namespace: {}", namespace));

                Ok(namespace)
            }
            None => Err(JsValue::from_str(
                "please put the namespace as a second parameter",
            )),
        };

        let storage = match storage_type {
            "memory" => Storage::Memory(MemoryStorage::new().unwrap()),
            "localstorage" => Storage::Local(LocalStorage::new(get_namespace()?).unwrap()),
            "sessionstorage" => Storage::Session(SessionStorage::new(get_namespace()?).unwrap()),
            _ => {
                let e = JsValue::from_str(
                    "storage type options: memory | localstorage | sessionstorage",
                );
                return Err(e);
            }
        };

        let storage = Some(storage);
        log("[GlueSQL] ready to use :)");

        Ok(Self { storage })
    }

    pub fn execute(&mut self, sql: String) -> Result<JsValue, JsValue> {
        let mut payloads = vec![];

        let queries = gluesql_core::parse(&sql).map_err(|error| {
            let message = format!("{:?}", error);

            JsValue::from_serde(&message).unwrap()
        })?;

        macro_rules! execute {
            ($storage: ident, $query: ident) => {
                match gluesql_core::execute($storage, $query) {
                    Ok((storage, payload)) => {
                        payloads.push(payload);

                        (storage, Ok(()))
                    }
                    Err((storage, error)) => (storage, Err(JsValue::from_serde(&error).unwrap())),
                }
            };
        }

        for query in queries.iter() {
            let storage = self.storage.take().unwrap();
            match storage {
                Storage::Memory(storage) => {
                    let (storage, result) = execute!(storage, query);

                    self.storage = Some(Storage::Memory(storage));
                    result?;
                }
                Storage::Local(storage) => {
                    let (storage, result) = execute!(storage, query);

                    self.storage = Some(Storage::Local(storage));
                    result?;
                }
                Storage::Session(storage) => {
                    let (storage, result) = execute!(storage, query);

                    self.storage = Some(Storage::Session(storage));
                    result?;
                }
            }
        }

        Ok(convert(payloads))
    }
}
