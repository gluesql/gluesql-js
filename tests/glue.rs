use gluesql::MemoryStorage;
use gluesql_core::tests::*;
use gluesql_core::{execute, generate_tests, Payload, Query, Result};

struct MemoryTester {
    storage: Option<MemoryStorage>,
}

impl Tester for MemoryTester {
    fn new(namespace: &str) -> Self {
        let storage = MemoryStorage::new().unwrap_or_else(|_| {
            panic!("MemoryStorage::new {}", namespace);
        });
        let storage = Some(storage);

        Self { storage }
    }

    fn execute(&mut self, query: &Query) -> Result<Payload> {
        let storage = self.storage.take().unwrap();

        match execute(storage, query) {
            Ok((storage, payload)) => {
                self.storage = Some(storage);

                Ok(payload)
            }
            Err((storage, error)) => {
                self.storage = Some(storage);

                Err(error)
            }
        }
    }
}

generate_tests!(MemoryTester);
