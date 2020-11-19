use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use gluesql_core::parser::ast::{ColumnDef, ColumnOption, ColumnOptionDef, Value as AstValue};
use gluesql_core::{
    AlterTable, AlterTableError, Error, MutResult, Result, Row, RowIter, Schema, Store, StoreMut,
    Value,
};

use wasm_bindgen::prelude::*;

macro_rules! try_into {
    ($expr: expr) => {
        $expr.map_err(|e| Error::Storage(Box::new(e)))?
    };
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

macro_rules! try_self {
    ($self: expr, $expr: expr) => {
        match $expr {
            Err(e) => {
                return Err(($self, e.into()));
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
                format!("__gluesql-v0.2__/{}/id/{}", self.namespace, table_name)
            }

            fn get_schema_prefix(&self, table_name: &str) -> String {
                format!("__gluesql-v0.2__/{}/schema/{}", self.namespace, table_name)
            }

            fn get_data_prefix(&self, table_name: &str) -> String {
                format!("__gluesql-v0.2__/{}/data/{}", self.namespace, table_name)
            }
        }

        #[async_trait]
        impl StoreMut<$StorageKey> for $Storage {
            async fn generate_id(self, table_name: &str) -> MutResult<Self, $StorageKey> {
                let prefix = self.get_id_prefix(table_name);
                let table_name = table_name.to_string();

                let key = match $get_item(&prefix).as_string() {
                    Some(v) => {
                        let $StorageKey { id, .. } = try_into!(self, serde_json::from_str(&v));

                        $StorageKey {
                            table_name,
                            id: id + 1,
                        }
                    }
                    None => $StorageKey { table_name, id: 1 },
                };

                let serialized = try_into!(self, serde_json::to_string(&key));
                $set_item(&prefix, &serialized);

                Ok((self, key))
            }

            async fn insert_schema(self, schema: &Schema) -> MutResult<Self, ()> {
                let prefix = self.get_schema_prefix(&schema.table_name);
                let schema = try_into!(self, serde_json::to_string(&schema));

                $set_item(&prefix, &schema);

                Ok((self, ()))
            }

            async fn delete_schema(self, table_name: &str) -> MutResult<Self, ()> {
                let schema_prefix = self.get_schema_prefix(table_name);
                let data_prefix = self.get_data_prefix(table_name);

                $remove_item(&schema_prefix);
                $remove_item(&data_prefix);

                Ok((self, ()))
            }

            async fn insert_data(self, key: &$StorageKey, row: Row) -> MutResult<Self, ()> {
                let prefix = self.get_data_prefix(&key.table_name);
                let item = (key.id, row);

                let mut items = match $get_item(&prefix).as_string() {
                    Some(v) => {
                        let items: Vec<(u64, Row)> = try_into!(self, serde_json::from_str(&v));

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

                let items = try_into!(self, serde_json::to_string(&items));
                $set_item(&prefix, &items);

                Ok((self, ()))
            }

            async fn delete_data(self, key: &$StorageKey) -> MutResult<Self, ()> {
                let prefix = self.get_data_prefix(&key.table_name);

                let mut items = match $get_item(&prefix).as_string() {
                    Some(v) => {
                        let items: Vec<(u64, Row)> = try_into!(self, serde_json::from_str(&v));

                        items
                    }
                    None => vec![],
                };

                if let Some(index) = items.iter().position(|(id, _)| id == &key.id) {
                    items.remove(index);
                }

                let items = try_into!(self, serde_json::to_string(&items));
                $set_item(&prefix, &items);

                Ok((self, ()))
            }
        }

        #[async_trait]
        impl Store<$StorageKey> for $Storage {
            async fn fetch_schema(&self, table_name: &str) -> Result<Option<Schema>> {
                let prefix = self.get_schema_prefix(table_name);

                let schema = match $get_item(&prefix).as_string() {
                    Some(schema) => Some(try_into!(serde_json::from_str(&schema))),
                    None => None,
                };

                Ok(schema)
            }

            async fn scan_data(&self, table_name: &str) -> Result<RowIter<$StorageKey>> {
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

        #[async_trait]
        impl AlterTable for $Storage {
            async fn rename_schema(
                self,
                table_name: &str,
                new_table_name: &str,
            ) -> MutResult<Self, ()> {
                // update schema
                let schema_prefix = self.get_schema_prefix(table_name);

                let schema = try_self!(
                    self,
                    $get_item(&schema_prefix)
                        .as_string()
                        .ok_or(AlterTableError::TableNotFound(table_name.to_string()))
                );
                let mut schema: Schema = try_into!(self, serde_json::from_str(&schema));

                schema.table_name = new_table_name.to_string();

                let new_schema_prefix = self.get_schema_prefix(new_table_name);
                let schema = try_into!(self, serde_json::to_string(&schema));
                $set_item(&new_schema_prefix, &schema);
                $remove_item(&schema_prefix);

                // migrate data
                let data_prefix = self.get_data_prefix(table_name);
                let new_data_prefix = self.get_data_prefix(new_table_name);

                if let Some(data) = $get_item(&data_prefix).as_string() {
                    $set_item(&new_data_prefix, &data);
                }

                Ok((self, ()))
            }

            async fn rename_column(
                self,
                table_name: &str,
                old_column_name: &str,
                new_column_name: &str,
            ) -> MutResult<Self, ()> {
                let prefix = self.get_schema_prefix(table_name);
                let schema = try_self!(
                    self,
                    $get_item(&prefix)
                        .as_string()
                        .ok_or(AlterTableError::TableNotFound(table_name.to_string()))
                );
                let mut schema: Schema = try_into!(self, serde_json::from_str(&schema));

                let i = schema
                    .column_defs
                    .iter()
                    .position(|column_def| column_def.name.value == old_column_name)
                    .ok_or(AlterTableError::RenamingColumnNotFound);
                let i = try_self!(self, i);

                schema.column_defs[i].name.value = new_column_name.to_string();

                let schema = try_into!(self, serde_json::to_string(&schema));
                $set_item(&prefix, &schema);

                Ok((self, ()))
            }

            async fn add_column(
                self,
                table_name: &str,
                column_def: &ColumnDef,
            ) -> MutResult<Self, ()> {
                let schema_prefix = self.get_schema_prefix(table_name);
                let schema = try_self!(
                    self,
                    $get_item(&schema_prefix)
                        .as_string()
                        .ok_or(AlterTableError::TableNotFound(table_name.to_string()))
                );
                let mut schema: Schema = try_into!(self, serde_json::from_str(&schema));

                if schema
                    .column_defs
                    .iter()
                    .any(|ColumnDef { name, .. }| name.value == column_def.name.value)
                {
                    let adding_column = column_def.name.value.to_string();

                    return Err((
                        self,
                        AlterTableError::AddingColumnAlreadyExists(adding_column).into(),
                    ));
                }

                schema.column_defs.push(column_def.clone());

                let ColumnDef {
                    options, data_type, ..
                } = column_def;

                let nullable = options
                    .iter()
                    .any(|ColumnOptionDef { option, .. }| option == &ColumnOption::Null);
                let default = options
                    .iter()
                    .filter_map(|ColumnOptionDef { option, .. }| match option {
                        ColumnOption::Default(expr) => Some(expr),
                        _ => None,
                    })
                    .map(|expr| Value::from_expr(&data_type, nullable, expr))
                    .next();

                let value = match (default, nullable) {
                    (Some(value), _) => try_self!(self, value),
                    (None, true) => try_self!(
                        self,
                        Value::from_data_type(&data_type, nullable, &AstValue::Null)
                    ),
                    (None, false) => {
                        return Err((
                            self,
                            AlterTableError::DefaultValueRequired(column_def.to_string()).into(),
                        ));
                    }
                };

                let schema = try_into!(self, serde_json::to_string(&schema));
                $set_item(&schema_prefix, &schema);

                let data_prefix = self.get_data_prefix(table_name);
                let items = match $get_item(&data_prefix).as_string() {
                    Some(v) => {
                        let items: Vec<(u64, Row)> = try_into!(self, serde_json::from_str(&v));

                        items
                    }
                    None => vec![],
                };

                let items: Vec<(u64, Row)> = items
                    .into_iter()
                    .map(|(id, mut row)| {
                        row.0.push(value.clone());

                        (id, row)
                    })
                    .collect();

                let items = try_into!(self, serde_json::to_string(&items));
                $set_item(&data_prefix, &items);

                Ok((self, ()))
            }

            async fn drop_column(
                self,
                table_name: &str,
                column_name: &str,
                if_exists: bool,
            ) -> MutResult<Self, ()> {
                let schema_prefix = self.get_schema_prefix(table_name);
                let schema = try_self!(
                    self,
                    $get_item(&schema_prefix)
                        .as_string()
                        .ok_or(AlterTableError::TableNotFound(table_name.to_string()))
                );
                let mut schema: Schema = try_into!(self, serde_json::from_str(&schema));

                let index = schema
                    .column_defs
                    .iter()
                    .position(|ColumnDef { name, .. }| name.value == column_name);

                let index = match (index, if_exists) {
                    (Some(index), _) => index,
                    (None, true) => {
                        return Ok((self, ()));
                    }
                    (None, false) => {
                        return Err((
                            self,
                            AlterTableError::DroppingColumnNotFound(column_name.to_string()).into(),
                        ));
                    }
                };

                schema.column_defs.remove(index);
                let schema = try_into!(self, serde_json::to_string(&schema));
                $set_item(&schema_prefix, &schema);

                let data_prefix = self.get_data_prefix(table_name);
                let items = match $get_item(&data_prefix).as_string() {
                    Some(v) => {
                        let items: Vec<(u64, Row)> = try_into!(self, serde_json::from_str(&v));

                        items
                    }
                    None => vec![],
                };

                let items: Vec<(u64, Row)> = items
                    .into_iter()
                    .map(|(id, mut row)| {
                        row.0.remove(index);

                        (id, row)
                    })
                    .collect();

                let items = try_into!(self, serde_json::to_string(&items));
                $set_item(&data_prefix, &items);

                Ok((self, ()))
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
