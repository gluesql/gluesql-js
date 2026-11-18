#![cfg(all(target_arch = "wasm32", not(feature = "opfs")))]

wasm_bindgen_test_configure!(run_in_browser);

use {
    gloo_utils::format::JsValueSerdeExt,
    gluesql_js::Glue,
    js_sys::JSON,
    serde_json::{Value as Json, json},
    wasm_bindgen::JsValue,
    wasm_bindgen_futures::JsFuture,
    wasm_bindgen_test::*,
};

async fn run(glue: &mut Glue, sql: &str) -> Json {
    JsFuture::from(glue.query(sql.to_owned()))
        .await
        .unwrap()
        .into_serde()
        .unwrap()
}

async fn dump(glue: &mut Glue) -> String {
    JsFuture::from(glue.dump(&JsValue::UNDEFINED))
        .await
        .unwrap()
        .as_string()
        .unwrap()
}

#[wasm_bindgen_test]
async fn dump_and_restore_keep_engines() {
    let mut glue = Glue::new();

    run(
        &mut glue,
        "
        CREATE TABLE DumpMem (id INTEGER) ENGINE = memory;
        CREATE TABLE DumpLoc (id INTEGER, name TEXT) ENGINE = localStorage;
        CREATE TABLE DumpSes (id INTEGER) ENGINE = sessionStorage;
        INSERT INTO DumpMem VALUES (1);
        INSERT INTO DumpLoc VALUES (2, 'it''s'), (3, NULL);
        INSERT INTO DumpSes VALUES (4);
        ",
    )
    .await;

    let sql = dump(&mut glue).await;

    for engine in ["memory", "localStorage", "sessionStorage"] {
        assert!(sql.contains(&format!("ENGINE = {engine};")), "{sql}");
    }

    // `dump({ schema: false })` writes the rows only.
    let options = JSON::parse(r#"{ "schema": false }"#).unwrap();
    let data = JsFuture::from(glue.dump(&options))
        .await
        .unwrap()
        .as_string()
        .unwrap();

    assert_eq!(
        data,
        [
            r#"INSERT INTO "DumpLoc" ("id", "name") VALUES (2, 'it''s'), (3, NULL);"#,
            r#"INSERT INTO "DumpMem" ("id") VALUES (1);"#,
            r#"INSERT INTO "DumpSes" ("id") VALUES (4);"#,
            "",
        ]
        .join("\n")
    );

    run(&mut glue, "DROP TABLE DumpMem, DumpLoc, DumpSes;").await;
    assert_eq!(dump(&mut glue).await, "");

    JsFuture::from(glue.restore(&sql)).await.unwrap();
    assert_eq!(dump(&mut glue).await, sql);

    let expected = json!([
        {
            "type": "SELECT",
            "rows": [
                { "id": 2, "name": "it's" },
                { "id": 3, "name": null },
            ],
        },
    ]);
    assert_eq!(
        run(&mut glue, "SELECT * FROM DumpLoc ORDER BY id").await,
        expected
    );

    run(&mut glue, "DROP TABLE DumpMem, DumpLoc, DumpSes;").await;
}

#[wasm_bindgen_test]
async fn failed_restore_drops_created_tables() {
    let mut glue = Glue::new();

    run(
        &mut glue,
        "
        CREATE TABLE KeepLoc (id INTEGER) ENGINE = localStorage;
        INSERT INTO KeepLoc VALUES (1);
        ",
    )
    .await;
    let before = dump(&mut glue).await;

    let error = JsFuture::from(glue.restore(
        "
        CREATE TABLE GoneLoc (id INTEGER PRIMARY KEY) ENGINE = localStorage;
        CREATE TABLE GoneMem (id INTEGER);
        INSERT INTO GoneLoc VALUES (1);
        INSERT INTO GoneLoc VALUES (1);
        ",
    ))
    .await
    .unwrap_err()
    .as_string()
    .unwrap();

    assert!(error.contains("duplicate entry"), "{error}");
    assert_eq!(dump(&mut glue).await, before);

    let error = JsFuture::from(glue.restore("INSERT INTO KeepLoc VALUES (2);"))
        .await
        .unwrap_err()
        .as_string()
        .unwrap();

    assert!(error.contains("restore: only CREATE TABLE"), "{error}");
    assert_eq!(dump(&mut glue).await, before);

    run(&mut glue, "DROP TABLE KeepLoc;").await;
}
