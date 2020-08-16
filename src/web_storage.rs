use serde::{Deserialize, Serialize};

use gluesql_core::{Error, MutResult, Result, Row, RowIter, Schema, Store, StoreError, StoreMut};

use wasm_bindgen::prelude::*;

macro_rules! try_into {
    ($expr: expr) => {
        $expr.map_err(|e| Error::Storage(Box::new(e)))?
    };
}

macro_rules! try_self {
    ($self: expr, $expr: expr) => {
        match $expr {
            Err(e) => {
                let e = Error::Storage(Box::new(e));

                return Err(($self, e));
            }
            Ok(v) => v,
        }
    };
}

macro_rules! generate_storage_code {
    ($type: tt, $Storage: ident, $StorageKey: ident, $get_item: ident, $set_item: ident, $remove_item: ident) => {
        #[wasm_bindgen]
        extern "C" {
            #[wasm_bindgen(js_namespace = $type, js_name = getItem)]
            fn $get_item(k: &str) -> JsValue;

            #[wasm_bindgen(js_namespace = $type, js_name = setItem)]
            fn $set_item(k: &str, v: &str);

            #[wasm_bindgen(js_namespace = $type, js_name = removeItem)]
            fn $remove_item(k: &str);
        }

        pub struct $Storage {
            namespace: String,
        }

        #[derive(Clone, Debug, Serialize, Deserialize)]
        pub struct $StorageKey {
            pub table_name: String,
            pub id: u64,
        }

        impl $Storage {
            pub fn new(namespace: String) -> Result<Self> {
                Ok(Self { namespace })
            }

            fn get_id_prefix(&self, table_name: &str) -> String {
                format!("__gluesql-v0.1__/{}/id/{}", self.namespace, table_name)
            }

            fn get_schema_prefix(&self, table_name: &str) -> String {
                format!("__gluesql-v0.1__/{}/schema/{}", self.namespace, table_name)
            }

            fn get_data_prefix(&self, table_name: &str) -> String {
                format!("__gluesql-v0.1__/{}/data/{}", self.namespace, table_name)
            }
        }

        impl StoreMut<$StorageKey> for $Storage {
            fn generate_id(self, table_name: &str) -> MutResult<Self, $StorageKey> {
                let prefix = self.get_id_prefix(table_name);
                let table_name = table_name.to_string();

                let key = match $get_item(&prefix).as_string() {
                    Some(v) => {
                        let $StorageKey { id, .. } = try_self!(self, serde_json::from_str(&v));

                        $StorageKey {
                            table_name,
                            id: id + 1,
                        }
                    }
                    None => $StorageKey { table_name, id: 1 },
                };

                let serialized = try_self!(self, serde_json::to_string(&key));
                $set_item(&prefix, &serialized);

                Ok((self, key))
            }

            fn insert_schema(self, schema: &Schema) -> MutResult<Self, ()> {
                let prefix = self.get_schema_prefix(&schema.table_name);
                let schema = try_self!(self, serde_json::to_string(&schema));

                $set_item(&prefix, &schema);

                Ok((self, ()))
            }

            fn delete_schema(self, table_name: &str) -> MutResult<Self, ()> {
                let schema_prefix = self.get_schema_prefix(table_name);
                let data_prefix = self.get_data_prefix(table_name);

                $remove_item(&schema_prefix);
                $remove_item(&data_prefix);

                Ok((self, ()))
            }

            fn insert_data(self, key: &$StorageKey, row: Row) -> MutResult<Self, Row> {
                let prefix = self.get_data_prefix(&key.table_name);
                let item = (key.id, row.clone());

                let mut items = match $get_item(&prefix).as_string() {
                    Some(v) => {
                        let items: Vec<(u64, Row)> = try_self!(self, serde_json::from_str(&v));

                        items
                    }
                    None => vec![],
                };

                let items = match items.iter().position(|(id, _)| id == &key.id) {
                    Some(index) => {
                        items[index] = item;

                        items
                    }
                    None => {
                        items.push(item);

                        items
                    }
                };

                let items = try_self!(self, serde_json::to_string(&items));
                $set_item(&prefix, &items);

                Ok((self, row))
            }

            fn delete_data(self, key: &$StorageKey) -> MutResult<Self, ()> {
                let prefix = self.get_data_prefix(&key.table_name);

                let mut items = match $get_item(&prefix).as_string() {
                    Some(v) => {
                        let items: Vec<(u64, Row)> = try_self!(self, serde_json::from_str(&v));

                        items
                    }
                    None => vec![],
                };

                if let Some(index) = items.iter().position(|(id, _)| id == &key.id) {
                    items.remove(index);
                }

                let items = try_self!(self, serde_json::to_string(&items));
                $set_item(&prefix, &items);

                Ok((self, ()))
            }
        }

        impl Store<$StorageKey> for $Storage {
            fn fetch_schema(&self, table_name: &str) -> Result<Schema> {
                let prefix = self.get_schema_prefix(table_name);

                let schema = $get_item(&prefix)
                    .as_string()
                    .ok_or(StoreError::SchemaNotFound)?;
                let schema = try_into!(serde_json::from_str(&schema));

                Ok(schema)
            }

            fn scan_data(&self, table_name: &str) -> Result<RowIter<$StorageKey>> {
                let prefix = self.get_data_prefix(table_name);

                let items = match $get_item(&prefix).as_string() {
                    Some(items) => {
                        let items: Vec<(u64, Row)> = try_into!(serde_json::from_str(&items));

                        items
                            .into_iter()
                            .map(|(id, row)| {
                                let key = $StorageKey {
                                    table_name: table_name.to_string(),
                                    id,
                                };

                                (key, row)
                            })
                            .collect()
                    }
                    None => vec![],
                };

                let items = items.into_iter().map(Ok);

                Ok(Box::new(items))
            }
        }
    };
}

generate_storage_code!(
    localStorage,
    LocalStorage,
    LocalKey,
    lget_item,
    lset_item,
    lremove_item
);

generate_storage_code!(
    sessionStorage,
    SessionStorage,
    SessionKey,
    sget_item,
    sset_item,
    sremove_item
);
