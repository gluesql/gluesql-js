use std::cell::RefCell;
use std::rc::Rc;

use gluesql::web_storage::{LocalKey, LocalStorage};
use gluesql_core::tests::*;
use gluesql_core::*;

use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

struct LocalTester {
    storage: Rc<RefCell<Option<LocalStorage>>>,
}

impl Tester<LocalKey, LocalStorage> for LocalTester {
    fn new(namespace: &str) -> Self {
        let storage = LocalStorage::new(namespace.to_string()).unwrap_or_else(|_| {
            panic!("LocalStorage::new {}", namespace);
        });
        let storage = Rc::new(RefCell::new(Some(storage)));

        Self { storage }
    }

    fn get_cell(&mut self) -> Rc<RefCell<Option<LocalStorage>>> {
        Rc::clone(&self.storage)
    }
}

generate_tests!(wasm_bindgen_test, LocalTester);
