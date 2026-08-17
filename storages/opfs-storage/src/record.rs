use {
    gluesql_core::{
        data::{Key, Schema, Value},
        error::{Error, Result},
    },
    serde::{Deserialize, Serialize},
};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum StoredRecord {
    Schema {
        schema: Box<Schema>,
    },
    Row {
        table: String,
        key: Key,
        values: Vec<Value>,
    },
}

pub fn encode(record: &StoredRecord) -> Result<Vec<u8>> {
    bincode::serialize(record)
        .map_err(|error| storage_error(format!("record encode failed: {error}")))
}

pub fn decode(bytes: &[u8]) -> Result<StoredRecord> {
    bincode::deserialize(bytes)
        .map_err(|error| storage_error(format!("record decode failed: {error}")))
}

fn storage_error(message: impl Into<String>) -> Error {
    Error::StorageMsg(format!("opfs-storage: {}", message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_roundtrip() {
        let record = StoredRecord::Row {
            table: "Foo".to_owned(),
            key: Key::I64(1),
            values: vec![Value::I64(1), Value::Str("hello".to_owned())],
        };

        assert_eq!(decode(&encode(&record).unwrap()).unwrap(), record);
    }
}
