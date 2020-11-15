use std::cell::RefCell;
use std::rc::Rc;

use gluesql::memory_storage::{DataKey, MemoryStorage};
use gluesql_core::tests::*;
use gluesql_core::*;

use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

struct MemoryTester {
    storage: Rc<RefCell<Option<MemoryStorage>>>,
}

impl Tester<DataKey, MemoryStorage> for MemoryTester {
    fn new(namespace: &str) -> Self {
        let storage = MemoryStorage::new().unwrap_or_else(|_| {
            panic!("MemoryStorage::new {}", namespace);
        });
        let storage = Rc::new(RefCell::new(Some(storage)));

        Self { storage }
    }

    fn get_cell(&mut self) -> Rc<RefCell<Option<MemoryStorage>>> {
        Rc::clone(&self.storage)
    }
}

generate_tests!(wasm_bindgen_test, MemoryTester);
