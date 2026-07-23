#![cfg(target_arch = "wasm32")]
#![deny(clippy::str_to_string)]

use {
    gloo_storage::{LocalStorage, SessionStorage, Storage, errors::StorageError},
    gluesql_core::{
        ast::ColumnUniqueOption,
        data::{Key, Schema, Value},
        error::{Error, Result},
        store::{
            AlterTable, CustomFunction, CustomFunctionMut, Index, IndexMut, Metadata, Planner,
            RowIter, Store, StoreMut, Transaction,
        },
    },
    serde::{Deserialize, Serialize},
    uuid::Uuid,
};

/// gluesql-schema-names -> {Vec<String>}
const TABLE_NAMES_PATH: &str = "gluesql-schema-names";

/// gluesql-schema/{schema_name} -> {Schema}
const SCHEMA_PATH: &str = "gluesql-schema";

/// gluesql-data/{table_name} -> {Vec<(Key, Vec<Value>)>}
const DATA_PATH: &str = "gluesql-data";

#[derive(Clone, Copy, Default, Serialize, Deserialize)]
pub enum WebStorageType {
    #[default]
    Local,
    Session,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct WebStorage {
    storage_type: WebStorageType,
}

impl WebStorage {
    pub fn new(storage_type: WebStorageType) -> Self {
        Self { storage_type }
    }

    pub fn raw(&self) -> web_sys::Storage {
        match self.storage_type {
            WebStorageType::Local => LocalStorage::raw(),
            WebStorageType::Session => SessionStorage::raw(),
        }
    }

    pub fn get<T>(&self, key: impl AsRef<str>) -> Result<Option<T>>
    where
        T: for<'de> Deserialize<'de>,
    {
        let value = match self.storage_type {
            WebStorageType::Local => LocalStorage::get(key),
            WebStorageType::Session => SessionStorage::get(key),
        };

        match value {
            Ok(value) => Ok(Some(value)),
            Err(StorageError::KeyNotFound(_)) => Ok(None),
            Err(e) => Err(Error::StorageMsg(e.to_string())),
        }
    }

    pub fn set<T>(&self, key: impl AsRef<str>, value: T) -> Result<()>
    where
        T: Serialize,
    {
        match self.storage_type {
            WebStorageType::Local => LocalStorage::set(key, value),
            WebStorageType::Session => SessionStorage::set(key, value),
        }
        .map_err(|e| Error::StorageMsg(e.to_string()))
    }

    pub fn delete(&self, key: impl AsRef<str>) {
        match self.storage_type {
            WebStorageType::Local => LocalStorage::delete(key),
            WebStorageType::Session => SessionStorage::delete(key),
        }
    }
}

impl Store for WebStorage {
    fn fetch_all_schemas(&self) -> Result<Vec<Schema>> {
        let mut table_names: Vec<String> = self.get(TABLE_NAMES_PATH)?.unwrap_or_default();
        table_names.sort();

        table_names
            .iter()
            .filter_map(|table_name| self.get(format!("{SCHEMA_PATH}/{table_name}")).transpose())
            .collect::<Result<Vec<_>>>()
    }

    fn fetch_schema(&self, table_name: &str) -> Result<Option<Schema>> {
        self.get(format!("{SCHEMA_PATH}/{table_name}"))
    }

    fn fetch_data(&self, table_name: &str, target: &Key) -> Result<Option<Vec<Value>>> {
        let path = format!("{DATA_PATH}/{table_name}");
        let row = self
            .get::<Vec<(Key, Vec<Value>)>>(path)?
            .unwrap_or_default()
            .into_iter()
            .find_map(|(key, row)| (&key == target).then_some(row));

        Ok(row)
    }

    fn scan_data<'a>(&'a self, table_name: &str) -> Result<RowIter<'a>> {
        let path = format!("{DATA_PATH}/{table_name}");
        let mut rows = self
            .get::<Vec<(Key, Vec<Value>)>>(path)?
            .unwrap_or_default();

        match self.get(format!("{SCHEMA_PATH}/{table_name}"))? {
            Some(Schema {
                column_defs: Some(column_defs),
                ..
            }) if column_defs.iter().any(|column_def| {
                matches!(
                    column_def.unique,
                    Some(ColumnUniqueOption { is_primary: true })
                )
            }) =>
            {
                rows.sort_by(|(key_a, _), (key_b, _)| key_a.cmp(key_b));
            }
            _ => {}
        }

        Ok(Box::new(rows.into_iter().map(Ok)))
    }
}

impl StoreMut for WebStorage {
    fn insert_schema(&mut self, schema: &Schema) -> Result<()> {
        let mut table_names: Vec<String> = self.get(TABLE_NAMES_PATH)?.unwrap_or_default();
        table_names.push(schema.table_name.clone());

        self.set(TABLE_NAMES_PATH, table_names)?;
        self.set(format!("{SCHEMA_PATH}/{}", schema.table_name), schema)
    }

    fn delete_schema(&mut self, table_name: &str) -> Result<()> {
        let mut table_names: Vec<String> = self.get(TABLE_NAMES_PATH)?.unwrap_or_default();
        table_names
            .iter()
            .position(|name| name == table_name)
            .map(|i| table_names.remove(i));

        self.set(TABLE_NAMES_PATH, table_names)?;
        self.delete(format!("{SCHEMA_PATH}/{table_name}"));
        self.delete(format!("{DATA_PATH}/{table_name}"));
        Ok(())
    }

    fn append_data(&mut self, table_name: &str, new_rows: Vec<Vec<Value>>) -> Result<()> {
        let path = format!("{DATA_PATH}/{table_name}");
        let rows = self
            .get::<Vec<(Key, Vec<Value>)>>(&path)?
            .unwrap_or_default();
        let new_rows = new_rows.into_iter().map(|row| {
            let key = Key::Uuid(Uuid::new_v4().as_u128());

            (key, row)
        });

        let rows = rows.into_iter().chain(new_rows).collect::<Vec<_>>();

        self.set(path, rows)
    }

    fn insert_data(&mut self, table_name: &str, new_rows: Vec<(Key, Vec<Value>)>) -> Result<()> {
        let path = format!("{DATA_PATH}/{table_name}");
        let mut rows = self
            .get::<Vec<(Key, Vec<Value>)>>(&path)?
            .unwrap_or_default();

        for (key, row) in new_rows {
            if let Some(i) = rows.iter().position(|(k, _)| k == &key) {
                rows[i] = (key, row);
            } else {
                rows.push((key, row));
            }
        }

        self.set(path, rows)
    }

    fn delete_data(&mut self, table_name: &str, keys: Vec<Key>) -> Result<()> {
        let path = format!("{DATA_PATH}/{table_name}");
        let mut rows = self
            .get::<Vec<(Key, Vec<Value>)>>(&path)?
            .unwrap_or_default();

        for key in &keys {
            if let Some(i) = rows.iter().position(|(k, _)| k == key) {
                rows.remove(i);
            }
        }

        self.set(path, rows)
    }
}

impl AlterTable for WebStorage {}
impl Index for WebStorage {}
impl IndexMut for WebStorage {}
impl Transaction for WebStorage {}
impl Metadata for WebStorage {}
impl Planner for WebStorage {}
impl CustomFunction for WebStorage {}
impl CustomFunctionMut for WebStorage {}
