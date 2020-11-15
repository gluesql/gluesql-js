mod convert;
pub mod memory_storage;
mod utils;
pub mod web_storage;

use js_sys::Promise;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen_futures::future_to_promise;

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
    Empty,
    Memory(MemoryStorage),
    Local(LocalStorage),
    Session(SessionStorage),
}

#[wasm_bindgen]
pub struct Glue {
    storage: Rc<RefCell<Storage>>,
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

        let storage = Rc::new(RefCell::new(storage));
        log("[GlueSQL] ready to use :)");

        Ok(Self { storage })
    }

    pub fn execute(&mut self, sql: String) -> Promise {
        let cell = Rc::clone(&self.storage);

        future_to_promise(async move {
            let queries = gluesql_core::parse(&sql).map_err(|error| {
                let message = format!("{:?}", error);

                JsValue::from_serde(&message).unwrap()
            })?;

            let mut payloads = vec![];

            macro_rules! execute {
                ($storage: ident, $query: ident) => {
                    match gluesql_core::execute($storage, $query).await {
                        Ok((storage, payload)) => {
                            payloads.push(payload);

                            (storage, Ok(()))
                        }
                        Err((storage, error)) => {
                            (storage, Err(JsValue::from_serde(&error).unwrap()))
                        }
                    }
                };
            }

            let mut storage: Storage = cell.replace(Storage::Empty);

            for query in queries.iter() {
                match storage {
                    Storage::Memory(s) => {
                        let (s, result) = execute!(s, query);

                        storage = Storage::Memory(s);
                        result
                    }
                    Storage::Local(s) => {
                        let (s, result) = execute!(s, query);

                        storage = Storage::Local(s);
                        result
                    }
                    Storage::Session(s) => {
                        let (s, result) = execute!(s, query);

                        storage = Storage::Session(s);
                        result
                    }
                    Storage::Empty => Err(JsValue::from_str("unreachable empty storage")),
                }?;
            }

            cell.replace(storage);

            Ok(convert(payloads))
        })
    }
}
