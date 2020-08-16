use serde_json::map::Map;
use serde_json::value::Value as Json;
use wasm_bindgen::prelude::JsValue;

use gluesql_core::{Payload, Row, Value};

pub fn convert(payloads: Vec<Payload>) -> JsValue {
    let payloads = payloads
        .into_iter()
        .map(convert_payload)
        .map(|(query, data)| {
            let mut map = Map::new();

            map.insert("query".to_owned(), Json::String(query));
            map.insert("data".to_owned(), data);

            Json::Object(map)
        })
        .collect();
    let payloads = Json::Array(payloads);

    JsValue::from_serde(&payloads).unwrap()
}

fn convert_payload(payload: Payload) -> (String, Json) {
    match payload {
        Payload::Create => ("CREATE".to_owned(), Json::Null),
        Payload::Insert(row) => ("INSERT".to_owned(), convert_row(row)),
        Payload::Select(rows) => (
            "SELECT".to_owned(),
            Json::Array(rows.into_iter().map(convert_row).collect()),
        ),
        Payload::Delete(num) => ("DELETE".to_owned(), Json::from(num)),
        Payload::Update(num) => ("UPDATE".to_owned(), Json::from(num)),
        Payload::DropTable => ("DROP".to_owned(), Json::Null),
    }
}

fn convert_row(row: Row) -> Json {
    let Row(values) = row;

    Json::Array(values.into_iter().map(convert_value).collect())
}

fn convert_value(value: Value) -> Json {
    use Value::*;

    match value {
        Bool(v) | OptBool(Some(v)) => Json::Bool(v),
        I64(v) | OptI64(Some(v)) => Json::from(v),
        F64(v) | OptF64(Some(v)) => Json::from(v),
        Str(v) | OptStr(Some(v)) => Json::String(v),
        OptBool(None) | OptI64(None) | OptF64(None) | OptStr(None) | Empty => Json::Null,
    }
}
