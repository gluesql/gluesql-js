use async_trait::async_trait;
use boolinator::Boolinator;
use im::{vector, HashMap, Vector};

use gluesql_core::parser::ast::{ColumnDef, ColumnOption, ColumnOptionDef, Value as AstValue};
use gluesql_core::{
    AlterTable, AlterTableError, MutResult, Result, Row, RowIter, Schema, Store, StoreMut, Value,
};

pub struct MemoryStorage {
    schema_map: HashMap<String, Schema>,
    data_map: HashMap<String, Vector<(u64, Row)>>,
    id: u64,
}

#[derive(Clone, Debug)]
pub struct DataKey {
    pub table_name: String,
    pub id: u64,
}

impl MemoryStorage {
    pub fn new() -> Result<Self> {
        let schema_map = HashMap::new();
        let data_map = HashMap::new();

        Ok(Self {
            schema_map,
            data_map,
            id: 0,
        })
    }
}

#[async_trait(?Send)]
impl StoreMut<DataKey> for MemoryStorage {
    async fn generate_id(self, table_name: &str) -> MutResult<Self, DataKey> {
        let id = self.id + 1;
        let storage = Self {
            schema_map: self.schema_map,
            data_map: self.data_map,
            id,
        };

        let key = DataKey {
            table_name: table_name.to_string(),
            id,
        };

        Ok((storage, key))
    }

    async fn insert_schema(self, schema: &Schema) -> MutResult<Self, ()> {
        let table_name = schema.table_name.to_string();
        let schema_map = self.schema_map.update(table_name, schema.clone());
        let storage = Self {
            schema_map,
            data_map: self.data_map,
            id: self.id,
        };

        Ok((storage, ()))
    }

    async fn delete_schema(self, table_name: &str) -> MutResult<Self, ()> {
        let Self {
            mut schema_map,
            mut data_map,
            id,
        } = self;

        data_map.remove(table_name);
        schema_map.remove(table_name);
        let storage = Self {
            schema_map,
            data_map,
            id,
        };

        Ok((storage, ()))
    }

    async fn insert_data(self, key: &DataKey, row: Row) -> MutResult<Self, ()> {
        let DataKey { table_name, id } = key;
        let table_name = table_name.to_string();
        let item = (*id, row);
        let Self {
            schema_map,
            data_map,
            id: self_id,
        } = self;

        let (mut items, data_map) = match data_map.extract(&table_name) {
            Some(v) => v,
            None => (vector![], data_map),
        };

        let items = match items.iter().position(|(item_id, _)| item_id == id) {
            Some(index) => items.update(index, item),
            None => {
                items.push_back(item);

                items
            }
        };

        let data_map = data_map.update(table_name, items);
        let storage = Self {
            schema_map,
            data_map,
            id: self_id,
        };

        Ok((storage, ()))
    }

    async fn delete_data(self, key: &DataKey) -> MutResult<Self, ()> {
        let DataKey { table_name, id } = key;
        let table_name = table_name.to_string();
        let Self {
            schema_map,
            data_map,
            id: self_id,
        } = self;

        let (mut items, data_map) = match data_map.extract(&table_name) {
            Some(v) => v,
            None => (vector![], data_map),
        };

        if let Some(index) = items.iter().position(|(item_id, _)| item_id == id) {
            items.remove(index);
        };

        let data_map = data_map.update(table_name, items);
        let storage = Self {
            schema_map,
            data_map,
            id: self_id,
        };

        Ok((storage, ()))
    }
}

#[async_trait(?Send)]
impl Store<DataKey> for MemoryStorage {
    async fn fetch_schema(&self, table_name: &str) -> Result<Option<Schema>> {
        let schema = self.schema_map.get(table_name).cloned();

        Ok(schema)
    }

    async fn scan_data(&self, table_name: &str) -> Result<RowIter<DataKey>> {
        let items = match self.data_map.get(table_name) {
            Some(items) => items
                .iter()
                .map(|(id, row)| {
                    let key = DataKey {
                        table_name: table_name.to_string(),
                        id: *id,
                    };

                    (key, row.clone())
                })
                .collect(),
            None => vector![],
        };

        let items = items.into_iter().map(Ok);

        Ok(Box::new(items))
    }
}

macro_rules! try_into {
    ($self: expr, $expr: expr) => {
        match $expr {
            Err(e) => {
                return Err(($self, e.into()));
            }
            Ok(v) => v,
        }
    };
}

#[async_trait(?Send)]
impl AlterTable for MemoryStorage {
    async fn rename_schema(self, table_name: &str, new_table_name: &str) -> MutResult<Self, ()> {
        let mut schema = try_into!(
            self,
            self.schema_map
                .get(table_name)
                .ok_or_else(|| AlterTableError::TableNotFound(table_name.to_string()))
                .map(|s| s.clone())
        );

        schema.table_name = new_table_name.to_string();

        let Self {
            schema_map,
            data_map,
            id,
        } = self;

        let mut schema_map = schema_map.update(new_table_name.to_string(), schema);
        schema_map.remove(table_name);

        let (items, data_map) = match data_map.extract(table_name) {
            Some(v) => v,
            None => (vector![], data_map),
        };

        let data_map = data_map.update(new_table_name.to_string(), items);

        let storage = Self {
            schema_map,
            data_map,
            id,
        };

        Ok((storage, ()))
    }

    async fn rename_column(
        self,
        table_name: &str,
        old_column_name: &str,
        new_column_name: &str,
    ) -> MutResult<Self, ()> {
        let mut schema = try_into!(
            self,
            self.schema_map
                .get(table_name)
                .ok_or_else(|| AlterTableError::TableNotFound(table_name.to_string()))
                .map(|s| s.clone())
        );

        let i = schema
            .column_defs
            .iter()
            .position(|column_def| column_def.name.value == old_column_name)
            .ok_or(AlterTableError::RenamingColumnNotFound);
        let i = try_into!(self, i);

        schema.column_defs[i].name.value = new_column_name.to_string();

        let Self {
            schema_map,
            data_map,
            id,
        } = self;

        let schema_map = schema_map.update(table_name.to_string(), schema);

        let storage = Self {
            schema_map,
            data_map,
            id,
        };

        Ok((storage, ()))
    }

    async fn add_column(self, table_name: &str, column_def: &ColumnDef) -> MutResult<Self, ()> {
        let mut schema = try_into!(
            self,
            self.schema_map
                .get(table_name)
                .ok_or_else(|| AlterTableError::TableNotFound(table_name.to_string()))
                .map(|s| s.clone())
        );

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

        let Self {
            schema_map,
            data_map,
            id,
        } = self;

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

        let storage = Self {
            schema_map,
            data_map,
            id,
        };

        let value = match (default, nullable) {
            (Some(value), _) => try_into!(storage, value),
            (None, true) => try_into!(
                storage,
                Value::from_data_type(&data_type, nullable, &AstValue::Null)
            ),
            (None, false) => {
                return Err((
                    storage,
                    AlterTableError::DefaultValueRequired(column_def.to_string()).into(),
                ));
            }
        };

        let Self {
            schema_map,
            data_map,
            id,
        } = storage;

        let (items, data_map) = match data_map.extract(table_name) {
            Some(v) => v,
            None => (vector![], data_map),
        };

        let items = items
            .into_iter()
            .map(|(id, mut row)| {
                row.0.push(value.clone());

                (id, row)
            })
            .collect();

        let data_map = data_map.update(table_name.to_string(), items);
        let schema_map = schema_map.update(table_name.to_string(), schema);

        let storage = Self {
            schema_map,
            data_map,
            id,
        };

        Ok((storage, ()))
    }

    async fn drop_column(
        self,
        table_name: &str,
        column_name: &str,
        if_exists: bool,
    ) -> MutResult<Self, ()> {
        let Schema { column_defs, .. } = try_into!(
            self,
            self.schema_map
                .get(table_name)
                .ok_or_else(|| AlterTableError::TableNotFound(table_name.to_string()))
                .map(|s| s.clone())
        );

        let index = column_defs
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

        let Self {
            schema_map,
            data_map,
            id,
        } = self;

        let (items, data_map) = match data_map.extract(table_name) {
            Some(v) => v,
            None => (vector![], data_map),
        };

        let items = items
            .into_iter()
            .map(|(id, mut row)| {
                row.0.remove(index);

                (id, row)
            })
            .collect();

        let data_map = data_map.update(table_name.to_string(), items);

        let column_defs = column_defs
            .into_iter()
            .enumerate()
            .filter_map(|(i, v)| (i != index).as_some(v))
            .collect::<Vec<ColumnDef>>();

        let schema = Schema {
            table_name: table_name.to_string(),
            column_defs,
        };

        let schema_map = schema_map.update(table_name.to_string(), schema);

        let storage = Self {
            schema_map,
            data_map,
            id,
        };

        Ok((storage, ()))
    }
}
