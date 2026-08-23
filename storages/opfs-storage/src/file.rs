use {
    gluesql_core::error::{Error, Result},
    wasm_bindgen::{JsCast, JsValue},
    wasm_bindgen_futures::JsFuture,
    web_sys::{
        FileSystemDirectoryHandle, FileSystemFileHandle, FileSystemGetFileOptions,
        FileSystemReadWriteOptions, FileSystemSyncAccessHandle, WorkerGlobalScope,
    },
};

pub trait RandomAccessFile {
    fn size(&self) -> Result<u64>;

    fn read_at(&self, offset: u64, buffer: &mut [u8]) -> Result<usize>;

    fn write_at(&self, offset: u64, bytes: &[u8]) -> Result<()>;

    fn truncate(&self, len: u64) -> Result<()>;

    fn flush(&self) -> Result<()>;

    fn read_exact_at(&self, offset: u64, buffer: &mut [u8]) -> Result<()> {
        let mut read = 0;

        while read < buffer.len() {
            let count = self.read_at(offset + read as u64, &mut buffer[read..])?;

            if count == 0 {
                return Err(Error::StorageMsg(
                    "opfs-storage: unexpected end of file".to_owned(),
                ));
            }

            read += count;
        }

        Ok(())
    }

    fn read_all(&self) -> Result<Vec<u8>> {
        let size = usize::try_from(self.size()?)
            .map_err(|_| Error::StorageMsg("opfs-storage: file too large to load".to_owned()))?;
        let mut buffer = vec![0; size];
        self.read_exact_at(0, &mut buffer)?;

        Ok(buffer)
    }
}

pub struct SyncFile {
    handle: FileSystemSyncAccessHandle,
}

impl SyncFile {
    pub async fn open(name: &str) -> Result<Self> {
        let root = opfs_root().await?;

        let options = FileSystemGetFileOptions::new();
        options.set_create(true);

        let file: FileSystemFileHandle =
            JsFuture::from(root.get_file_handle_with_options(name, &options))
                .await
                .map_err(|error| js_error(&error))?
                .dyn_into()
                .map_err(|_| type_error("FileSystemFileHandle"))?;

        let handle: FileSystemSyncAccessHandle = JsFuture::from(file.create_sync_access_handle())
            .await
            .map_err(|error| js_error(&error))?
            .dyn_into()
            .map_err(|_| type_error("FileSystemSyncAccessHandle"))?;

        Ok(Self { handle })
    }
}

impl RandomAccessFile for SyncFile {
    fn size(&self) -> Result<u64> {
        self.handle
            .get_size()
            .map(|size| size as u64)
            .map_err(|error| js_error(&error))
    }

    fn read_at(&self, offset: u64, buffer: &mut [u8]) -> Result<usize> {
        let options = FileSystemReadWriteOptions::new();
        options.set_at(offset as f64);

        self.handle
            .read_with_u8_array_and_options(buffer, &options)
            .map(|read| read as usize)
            .map_err(|error| js_error(&error))
    }

    fn write_at(&self, offset: u64, bytes: &[u8]) -> Result<()> {
        let mut written = 0;

        while written < bytes.len() {
            let options = FileSystemReadWriteOptions::new();
            options.set_at((offset + written as u64) as f64);

            let count = self
                .handle
                .write_with_u8_array_and_options(&bytes[written..], &options)
                .map_err(|error| js_error(&error))? as usize;

            if count == 0 {
                return Err(Error::StorageMsg(
                    "opfs-storage: write made no progress".to_owned(),
                ));
            }

            written += count;
        }

        Ok(())
    }

    fn truncate(&self, len: u64) -> Result<()> {
        self.handle
            .truncate_with_f64(len as f64)
            .map_err(|error| js_error(&error))
    }

    fn flush(&self) -> Result<()> {
        self.handle.flush().map_err(|error| js_error(&error))
    }
}

impl Drop for SyncFile {
    fn drop(&mut self) {
        self.handle.close();
    }
}

async fn opfs_root() -> Result<FileSystemDirectoryHandle> {
    let scope: WorkerGlobalScope = js_sys::global().dyn_into().map_err(|_| {
        Error::StorageMsg(
            "opfs-storage: OPFS sync access requires a Dedicated Worker context".to_owned(),
        )
    })?;

    JsFuture::from(scope.navigator().storage().get_directory())
        .await
        .map_err(|error| js_error(&error))?
        .dyn_into()
        .map_err(|_| type_error("FileSystemDirectoryHandle"))
}

fn js_error(value: &JsValue) -> Error {
    Error::StorageMsg(format!("opfs-storage: {value:?}"))
}

fn type_error(expected: &str) -> Error {
    Error::StorageMsg(format!(
        "opfs-storage: unexpected type, expected {expected}"
    ))
}

#[cfg(test)]
pub(crate) mod tests {
    use {
        super::RandomAccessFile,
        gluesql_core::error::Result,
        std::sync::{Arc, Mutex},
    };

    #[derive(Clone, Default)]
    pub struct MemoryFile {
        bytes: Arc<Mutex<Vec<u8>>>,
    }

    impl RandomAccessFile for MemoryFile {
        fn size(&self) -> Result<u64> {
            Ok(self.bytes.lock().unwrap().len() as u64)
        }

        fn read_at(&self, offset: u64, buffer: &mut [u8]) -> Result<usize> {
            let bytes = self.bytes.lock().unwrap();
            let offset = usize::try_from(offset).unwrap();

            if offset >= bytes.len() {
                return Ok(0);
            }

            let len = buffer.len().min(bytes.len() - offset);
            buffer[..len].copy_from_slice(&bytes[offset..offset + len]);

            Ok(len)
        }

        fn write_at(&self, offset: u64, bytes: &[u8]) -> Result<()> {
            let offset = usize::try_from(offset).unwrap();
            let end = offset + bytes.len();
            let mut file = self.bytes.lock().unwrap();

            if file.len() < end {
                file.resize(end, 0);
            }

            file[offset..end].copy_from_slice(bytes);

            Ok(())
        }

        fn truncate(&self, len: u64) -> Result<()> {
            self.bytes
                .lock()
                .unwrap()
                .resize(usize::try_from(len).unwrap(), 0);

            Ok(())
        }

        fn flush(&self) -> Result<()> {
            Ok(())
        }
    }
}
