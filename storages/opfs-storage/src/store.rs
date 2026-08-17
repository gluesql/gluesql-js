use {
    crate::{
        file::SyncFile,
        page::PageId,
        pager::Pager,
        record::{self, StoredRecord},
    },
    gluesql_core::{
        data::{Key, Schema, Value},
        error::{Error, Result},
        store::{
            AlterTable, CustomFunction, CustomFunctionMut, Index, IndexMut, Metadata, Planner,
            RowIter, Store, StoreMut, Transaction,
        },
    },
    std::collections::{BTreeMap, BTreeSet, HashMap},
    uuid::Uuid,
};

struct Stored<T> {
    value: T,
    head: PageId,
}

pub struct OpfsStorage {
    pager: Pager<SyncFile, SyncFile>,
    schemas: HashMap<String, Stored<Schema>>,
    data: HashMap<String, BTreeMap<Key, Stored<Vec<Value>>>>,
}

impl OpfsStorage {
    pub async fn open(namespace: &str) -> Result<Self> {
        let database = SyncFile::open(&format!("{namespace}.db")).await?;
        let wal = SyncFile::open(&format!("{namespace}.wal")).await?;
        let pager = Pager::open(database, wal)?;
        let mut storage = Self {
            pager,
            schemas: HashMap::new(),
            data: HashMap::new(),
        };
        storage.load()?;

        Ok(storage)
    }

    fn load(&mut self) -> Result<()> {
        for (head, bytes) in self.pager.records()? {
            match record::decode(&bytes)? {
                StoredRecord::Schema { schema } => {
                    let table = schema.table_name.clone();
                    let stored = Stored {
                        value: *schema,
                        head,
                    };

                    if self.schemas.insert(table.clone(), stored).is_some() {
                        return Err(storage_error(format!(
                            "duplicate schema record for table {table}"
                        )));
                    }
                }
                StoredRecord::Row { table, key, values } => {
                    let stored = Stored {
                        value: values,
                        head,
                    };

                    if self
                        .data
                        .entry(table.clone())
                        .or_default()
                        .insert(key, stored)
                        .is_some()
                    {
                        return Err(storage_error(format!(
                            "duplicate row record for table {table}"
                        )));
                    }
                }
            }
        }

        for table in self.data.keys() {
            if !self.schemas.contains_key(table) {
                return Err(storage_error(format!(
                    "row record references missing table {table}"
                )));
            }
        }

        Ok(())
    }
}

impl Store for OpfsStorage {
    fn fetch_all_schemas(&self) -> Result<Vec<Schema>> {
        let mut schemas: Vec<Schema> = self
            .schemas
            .values()
            .map(|stored| stored.value.clone())
            .collect();
        schemas.sort_by(|a, b| a.table_name.cmp(&b.table_name));

        Ok(schemas)
    }

    fn fetch_schema(&self, table_name: &str) -> Result<Option<Schema>> {
        Ok(self
            .schemas
            .get(table_name)
            .map(|stored| stored.value.clone()))
    }

    fn fetch_data(&self, table_name: &str, target: &Key) -> Result<Option<Vec<Value>>> {
        Ok(self
            .data
            .get(table_name)
            .and_then(|rows| rows.get(target))
            .map(|stored| stored.value.clone()))
    }

    fn scan_data<'a>(&'a self, table_name: &str) -> Result<RowIter<'a>> {
        Ok(Box::new(
            self.data
                .get(table_name)
                .into_iter()
                .flatten()
                .map(|(key, stored)| Ok((key.clone(), stored.value.clone()))),
        ))
    }
}

impl StoreMut for OpfsStorage {
    fn insert_schema(&mut self, schema: &Schema) -> Result<()> {
        let table = schema.table_name.clone();
        let previous = self.schemas.get(&table).map(|stored| stored.head);
        let bytes = record::encode(&StoredRecord::Schema {
            schema: Box::new(schema.clone()),
        })?;
        let mut transaction = self.pager.transaction();

        if let Some(previous) = previous {
            transaction.free_record(previous)?;
        }

        let head = transaction.write_record(&bytes)?;
        transaction.commit()?;
        self.schemas.insert(
            table,
            Stored {
                value: schema.clone(),
                head,
            },
        );

        Ok(())
    }

    fn delete_schema(&mut self, table_name: &str) -> Result<()> {
        let mut heads = Vec::new();

        if let Some(schema) = self.schemas.get(table_name) {
            heads.push(schema.head);
        }

        if let Some(rows) = self.data.get(table_name) {
            heads.extend(rows.values().map(|stored| stored.head));
        }

        if !heads.is_empty() {
            let mut transaction = self.pager.transaction();

            for head in heads {
                transaction.free_record(head)?;
            }

            transaction.commit()?;
        }

        self.schemas.remove(table_name);
        self.data.remove(table_name);

        Ok(())
    }

    fn append_data(&mut self, table_name: &str, rows: Vec<Vec<Value>>) -> Result<()> {
        let rows: Vec<(Key, Vec<Value>, Vec<u8>)> = rows
            .into_iter()
            .map(|values| {
                let key = Key::Uuid(Uuid::new_v4().as_u128());
                let bytes = record::encode(&StoredRecord::Row {
                    table: table_name.to_owned(),
                    key: key.clone(),
                    values: values.clone(),
                })?;

                Ok((key, values, bytes))
            })
            .collect::<Result<_>>()?;
        let mut transaction = self.pager.transaction();
        let mut stored_rows = Vec::with_capacity(rows.len());

        for (key, values, bytes) in rows {
            let head = transaction.write_record(&bytes)?;
            stored_rows.push((
                key,
                Stored {
                    value: values,
                    head,
                },
            ));
        }

        transaction.commit()?;
        self.data
            .entry(table_name.to_owned())
            .or_default()
            .extend(stored_rows);

        Ok(())
    }

    fn insert_data(&mut self, table_name: &str, rows: Vec<(Key, Vec<Value>)>) -> Result<()> {
        let mut locations: BTreeMap<Key, PageId> = self
            .data
            .get(table_name)
            .into_iter()
            .flatten()
            .map(|(key, stored)| (key.clone(), stored.head))
            .collect();
        let mut transaction = self.pager.transaction();
        let mut stored_rows = BTreeMap::new();

        for (key, values) in rows {
            let bytes = record::encode(&StoredRecord::Row {
                table: table_name.to_owned(),
                key: key.clone(),
                values: values.clone(),
            })?;

            if let Some(previous) = locations.get(&key) {
                transaction.free_record(*previous)?;
            }

            let head = transaction.write_record(&bytes)?;
            locations.insert(key.clone(), head);

            stored_rows.insert(
                key,
                Stored {
                    value: values,
                    head,
                },
            );
        }

        transaction.commit()?;
        self.data
            .entry(table_name.to_owned())
            .or_default()
            .extend(stored_rows);

        Ok(())
    }

    fn delete_data(&mut self, table_name: &str, keys: Vec<Key>) -> Result<()> {
        let keys: BTreeSet<Key> = keys.into_iter().collect();
        let deleted: Vec<(Key, PageId)> = self
            .data
            .get(table_name)
            .into_iter()
            .flat_map(|rows| {
                keys.iter()
                    .filter_map(|key| rows.get(key).map(|stored| (key.clone(), stored.head)))
            })
            .collect();

        if !deleted.is_empty() {
            let mut transaction = self.pager.transaction();

            for (_, head) in &deleted {
                transaction.free_record(*head)?;
            }

            transaction.commit()?;
        }

        if let Some(rows) = self.data.get_mut(table_name) {
            for (key, _) in deleted {
                rows.remove(&key);
            }
        }

        Ok(())
    }
}

impl AlterTable for OpfsStorage {}
impl Index for OpfsStorage {}
impl IndexMut for OpfsStorage {}
impl Transaction for OpfsStorage {}
impl Metadata for OpfsStorage {}
impl Planner for OpfsStorage {}
impl CustomFunction for OpfsStorage {}
impl CustomFunctionMut for OpfsStorage {}

fn storage_error(message: impl Into<String>) -> Error {
    Error::StorageMsg(format!("opfs-storage: {}", message.into()))
}
