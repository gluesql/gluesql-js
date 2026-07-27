#![cfg(all(target_arch = "wasm32", feature = "opfs"))]

wasm_bindgen_test_configure!(run_in_dedicated_worker);

use {
    gloo_utils::format::JsValueSerdeExt,
    gluesql_js::Glue,
    serde_json::{Value as Json, json},
    wasm_bindgen_futures::JsFuture,
    wasm_bindgen_test::*,
};

#[wasm_bindgen_test]
async fn queries() {
    let mut glue = Glue::new();

    let sql = "
        CREATE TABLE Foo (id INTEGER, name TEXT);
        INSERT INTO Foo VALUES (1, 'hello'), (2, 'worker');
        SELECT * FROM Foo ORDER BY id;
    ";
    let actual: Json = JsFuture::from(glue.query(sql.to_owned()))
        .await
        .unwrap()
        .into_serde()
        .unwrap();
    let expected = json!([
        { "type": "CREATE TABLE" },
        { "type": "INSERT", "affected": 2 },
        {
            "type": "SELECT",
            "rows": [
                { "id": 1, "name": "hello" },
                { "id": 2, "name": "worker" },
            ],
        },
    ]);
    assert_eq!(actual, expected);
}

#[wasm_bindgen_test]
async fn error() {
    let mut glue = Glue::new();

    let error = JsFuture::from(glue.query("SELECT * FROM Missing".to_owned()))
        .await
        .unwrap_err();

    assert!(
        error
            .as_string()
            .unwrap()
            .contains("table not found: Missing")
    );
}
