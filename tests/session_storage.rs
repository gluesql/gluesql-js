use std::cell::RefCell;
use std::rc::Rc;

use gluesql::web_storage::{SessionKey, SessionStorage};
use gluesql_core::tests::*;
use gluesql_core::*;

use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

struct SessionTester {
    storage: Rc<RefCell<Option<SessionStorage>>>,
}

impl Tester<SessionKey, SessionStorage> for SessionTester {
    fn new(namespace: &str) -> Self {
        let storage = SessionStorage::new(namespace.to_string()).unwrap_or_else(|_| {
            panic!("SessionStorage::new {}", namespace);
        });
        let storage = Rc::new(RefCell::new(Some(storage)));

        Self { storage }
    }

    fn get_cell(&mut self) -> Rc<RefCell<Option<SessionStorage>>> {
        Rc::clone(&self.storage)
    }
}

generate_tests!(wasm_bindgen_test, SessionTester);
