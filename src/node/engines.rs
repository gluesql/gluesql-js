use {
    crate::node::storage::StorageConfig,
    gluesql_core::{
        data::{CustomFunction as StructCustomFunction, Key, Schema, Value},
        error::{Error, Result},
        store::{
            AlterTable, CustomFunction, CustomFunctionMut, GStore, GStoreMut, Index, IndexMut,
            MetaIter, Metadata, Planner, RowIter, Store, StoreMut, Transaction,
        },
    },
    gluesql_memory_storage::MemoryStorage,
    std::collections::{BTreeMap, HashMap},
};

/// Engine registered when a [`Glue`](super::Glue) instance is created.
pub const DEFAULT_ENGINE: &str = "memory";

/// Object-safe view of a storage backend.
pub trait Engine: GStore + GStoreMut {}
impl<T: GStore + GStoreMut> Engine for T {}

/// Named storage backends of one database instance.
///
/// Statements are routed to the backend that owns the table: the engine named
/// by the `ENGINE` clause when the table is created, and afterwards whichever
/// backend actually holds the schema. Ownership is not read back from
/// `Schema::engine`, because several storages drop that field when they reload
/// a schema from disk.
pub struct Engines {
    storages: BTreeMap<String, Box<dyn Engine>>,
    default_engine: Option<String>,
    functions: HashMap<String, StructCustomFunction>,
}

impl Default for Engines {
    fn default() -> Self {
        Self {
            storages: BTreeMap::from([(
                DEFAULT_ENGINE.to_owned(),
                Box::new(MemoryStorage::default()) as Box<dyn Engine>,
            )]),
            default_engine: Some(DEFAULT_ENGINE.to_owned()),
            functions: HashMap::new(),
        }
    }
}

impl Engines {
    /// Registers a backend under `engine`. The name is what the `ENGINE` clause
    /// of `CREATE TABLE` refers to.
    pub fn add(&mut self, engine: String, config: StorageConfig) -> Result<()> {
        if !is_identifier(&engine) {
            return Err(Error::StorageMsg(format!(
                "invalid engine name: {engine:?} (the ENGINE clause only accepts letters, digits and underscores)"
            )));
        }

        if self.storages.contains_key(&engine) {
            return Err(Error::StorageMsg(format!(
                "engine already exists: {engine} (remove it first to replace)"
            )));
        }

        self.storages.insert(engine, config.open()?);

        Ok(())
    }

    /// Unregisters a backend, leaving the data it owns untouched.
    pub fn remove(&mut self, engine: &str) -> Result<()> {
        if self.default_engine.as_deref() == Some(engine) {
            return Err(Error::StorageMsg(format!(
                "cannot remove the default engine: {engine} (call setDefaultEngine first)"
            )));
        }

        self.storages
            .remove(engine)
            .map(|_| ())
            .ok_or_else(|| self.not_found(engine))
    }

    /// Registered engine names, sorted alphabetically.
    pub fn names(&self) -> Vec<String> {
        self.storages.keys().cloned().collect()
    }

    pub fn default_engine(&self) -> Option<String> {
        self.default_engine.clone()
    }

    pub fn set_default(&mut self, engine: String) -> Result<()> {
        if !self.storages.contains_key(&engine) {
            return Err(self.not_found(&engine));
        }

        self.default_engine = Some(engine);

        Ok(())
    }

    fn not_found(&self, engine: &str) -> Error {
        Error::StorageMsg(format!(
            "engine not found: {engine} (registered: {})",
            self.storages
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        ))
    }

    /// Engine holding `table_name`, falling back to the default engine for
    /// tables that do not exist yet.
    fn engine_of(&self, table_name: &str) -> Result<&str> {
        for (engine, storage) in &self.storages {
            if storage.fetch_schema(table_name)?.is_some() {
                return Ok(engine);
            }
        }

        self.default_engine
            .as_deref()
            .ok_or_else(|| Error::StorageMsg(format!("engine not found for table: {table_name}")))
    }

    fn storage_of(&self, table_name: &str) -> Result<&dyn Engine> {
        let engine = self.engine_of(table_name)?;

        self.storages
            .get(engine)
            .map(Box::as_ref)
            .ok_or_else(|| Self::missing_storage(table_name))
    }

    fn storage_of_mut(&mut self, table_name: &str) -> Result<&mut Box<dyn Engine>> {
        let engine = self.engine_of(table_name)?.to_owned();

        self.storages
            .get_mut(&engine)
            .ok_or_else(|| Self::missing_storage(table_name))
    }

    fn missing_storage(table_name: &str) -> Error {
        Error::StorageMsg(format!("storage not found for table: {table_name}"))
    }
}

/// Names that cannot appear in an `ENGINE` clause are rejected on
/// registration; they would produce an engine no statement can reach.
fn is_identifier(engine: &str) -> bool {
    let mut chars = engine.chars();

    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && chars.all(|char| char.is_ascii_alphanumeric() || char == '_')
}

impl Store for Engines {
    fn fetch_schema(&self, table_name: &str) -> Result<Option<Schema>> {
        for (engine, storage) in &self.storages {
            if let Some(schema) = storage.fetch_schema(table_name)? {
                return Ok(Some(Schema {
                    engine: Some(engine.clone()),
                    ..schema
                }));
            }
        }

        Ok(None)
    }

    fn fetch_all_schemas(&self) -> Result<Vec<Schema>> {
        let mut schemas = Vec::new();

        for (engine, storage) in &self.storages {
            for schema in storage.fetch_all_schemas()? {
                schemas.push(Schema {
                    engine: Some(engine.clone()),
                    ..schema
                });
            }
        }

        schemas.sort_by(|a, b| a.table_name.cmp(&b.table_name));

        Ok(schemas)
    }

    fn fetch_data(&self, table_name: &str, key: &Key) -> Result<Option<Vec<Value>>> {
        self.storage_of(table_name)?.fetch_data(table_name, key)
    }

    fn scan_data<'a>(&'a self, table_name: &str) -> Result<RowIter<'a>> {
        self.storage_of(table_name)?.scan_data(table_name)
    }
}

impl StoreMut for Engines {
    fn insert_schema(&mut self, schema: &Schema) -> Result<()> {
        let engine = schema
            .engine
            .clone()
            .or_else(|| self.default_engine.clone())
            .ok_or_else(|| {
                Error::StorageMsg(format!("engine not found for table: {}", schema.table_name))
            })?;

        let schema = Schema {
            engine: Some(engine.clone()),
            ..schema.clone()
        };

        self.storages
            .get_mut(&engine)
            .ok_or_else(|| Self::missing_storage(&schema.table_name))?
            .insert_schema(&schema)
    }

    fn delete_schema(&mut self, table_name: &str) -> Result<()> {
        self.storage_of_mut(table_name)?.delete_schema(table_name)
    }

    fn append_data(&mut self, table_name: &str, rows: Vec<Vec<Value>>) -> Result<()> {
        self.storage_of_mut(table_name)?
            .append_data(table_name, rows)
    }

    fn insert_data(&mut self, table_name: &str, rows: Vec<(Key, Vec<Value>)>) -> Result<()> {
        self.storage_of_mut(table_name)?
            .insert_data(table_name, rows)
    }

    fn delete_data(&mut self, table_name: &str, keys: Vec<Key>) -> Result<()> {
        self.storage_of_mut(table_name)?
            .delete_data(table_name, keys)
    }
}

impl Transaction for Engines {
    fn begin(&mut self, autocommit: bool) -> Result<bool> {
        if !autocommit {
            return Err(Error::StorageMsg(
                "explicit transactions are not supported when engines are combined".to_owned(),
            ));
        }

        for storage in self.storages.values_mut() {
            storage.begin(autocommit)?;
        }

        Ok(true)
    }

    fn rollback(&mut self) -> Result<()> {
        for storage in self.storages.values_mut() {
            storage.rollback()?;
        }

        Ok(())
    }

    fn commit(&mut self) -> Result<()> {
        for storage in self.storages.values_mut() {
            storage.commit()?;
        }

        Ok(())
    }
}

impl Metadata for Engines {
    fn scan_table_meta(&self) -> Result<MetaIter> {
        let mut metas = Vec::new();

        for storage in self.storages.values() {
            for meta in storage.scan_table_meta()? {
                metas.push(meta?);
            }
        }

        metas.sort_by(|(a, _), (b, _)| a.cmp(b));

        Ok(Box::new(metas.into_iter().map(Ok)))
    }
}

impl CustomFunction for Engines {
    fn fetch_function<'a>(&'a self, func_name: &str) -> Result<Option<&'a StructCustomFunction>> {
        Ok(self.functions.get(&func_name.to_uppercase()))
    }

    fn fetch_all_functions(&self) -> Result<Vec<&StructCustomFunction>> {
        Ok(self.functions.values().collect())
    }
}

impl CustomFunctionMut for Engines {
    fn insert_function(&mut self, func: StructCustomFunction) -> Result<()> {
        self.functions.insert(func.func_name.to_uppercase(), func);

        Ok(())
    }

    fn delete_function(&mut self, func_name: &str) -> Result<()> {
        self.functions.remove(&func_name.to_uppercase());

        Ok(())
    }
}

impl AlterTable for Engines {}
impl Index for Engines {}
impl IndexMut for Engines {}
impl Planner for Engines {}
