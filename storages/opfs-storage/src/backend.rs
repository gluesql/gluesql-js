use {
    crate::file::RandomAccessFile,
    redb::StorageBackend,
    std::{fmt, io},
};

pub struct OpfsBackend<F> {
    file: F,
}

impl<F> OpfsBackend<F> {
    pub fn new(file: F) -> Self {
        Self { file }
    }
}

impl<F> fmt::Debug for OpfsBackend<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("OpfsBackend")
    }
}

unsafe impl<F> Send for OpfsBackend<F> {}
unsafe impl<F> Sync for OpfsBackend<F> {}

impl<F: RandomAccessFile + 'static> StorageBackend for OpfsBackend<F> {
    fn len(&self) -> Result<u64, io::Error> {
        self.file.size().map_err(|error| io_error(&error))
    }

    fn read(&self, offset: u64, len: usize) -> Result<Vec<u8>, io::Error> {
        let mut buffer = vec![0; len];
        self.file
            .read_exact_at(offset, &mut buffer)
            .map_err(|error| io_error(&error))?;

        Ok(buffer)
    }

    fn set_len(&self, len: u64) -> Result<(), io::Error> {
        self.file.truncate(len).map_err(|error| io_error(&error))
    }

    fn sync_data(&self, _eventual: bool) -> Result<(), io::Error> {
        self.file.flush().map_err(|error| io_error(&error))
    }

    fn write(&self, offset: u64, data: &[u8]) -> Result<(), io::Error> {
        self.file
            .write_at(offset, data)
            .map_err(|error| io_error(&error))
    }
}

fn io_error(error: &gluesql_core::error::Error) -> io::Error {
    io::Error::other(error.to_string())
}

#[cfg(test)]
mod tests {
    use {
        super::OpfsBackend,
        crate::file::tests::MemoryFile,
        gluesql_core::prelude::{Glue, Payload, Value},
        gluesql_redb_storage::RedbStorage,
    };

    #[test]
    fn redb_roundtrip_over_memory_file() {
        let backend = OpfsBackend::new(MemoryFile::default());
        let database = redb::Builder::new()
            .create_with_file_format_v3(true)
            .create_with_backend(backend)
            .unwrap();
        crate::initialize_format_version(&database).unwrap();
        let storage = RedbStorage::from_database(database).unwrap();
        let mut glue = Glue::new(storage);

        let payloads = glue
            .execute(
                "
                CREATE TABLE Foo (id INTEGER, name TEXT);
                INSERT INTO Foo VALUES (1, 'hello'), (2, 'redb');
                SELECT * FROM Foo ORDER BY id;
                ",
            )
            .unwrap();

        let Payload::Select { rows, .. } = &payloads[2] else {
            panic!("expected select payload");
        };
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1][1], Value::Str("redb".to_owned()));
    }
}
