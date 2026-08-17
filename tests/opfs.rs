#![cfg(all(target_arch = "wasm32", feature = "opfs"))]

wasm_bindgen_test_configure!(run_in_dedicated_worker);

use {
    gloo_utils::format::JsValueSerdeExt,
    gluesql_js::{Glue, load},
    serde_json::{Value as Json, json},
    wasm_bindgen_futures::JsFuture,
    wasm_bindgen_test::*,
};

async fn open(namespace: &str) -> Glue {
    load(Some(namespace.to_owned())).await.unwrap()
}

async fn run(glue: &mut Glue, sql: &str) -> Json {
    JsFuture::from(glue.query(sql.to_owned()))
        .await
        .unwrap()
        .into_serde()
        .unwrap()
}

#[wasm_bindgen_test]
async fn queries() {
    let mut glue = open("test-queries").await;

    let sql = "
        CREATE TABLE Foo (id INTEGER, name TEXT);
        INSERT INTO Foo VALUES (1, 'hello'), (2, 'worker');
        SELECT * FROM Foo ORDER BY id;
    ";
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
    assert_eq!(run(&mut glue, sql).await, expected);
}

#[wasm_bindgen_test]
async fn error() {
    let mut glue = open("test-error").await;

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

#[wasm_bindgen_test]
async fn join() {
    let mut glue = open("test-join").await;

    run(
        &mut glue,
        "
        CREATE TABLE Player (id INTEGER, name TEXT);
        CREATE TABLE Item (owner_id INTEGER, title TEXT);
        INSERT INTO Player VALUES (1, 'glue'), (2, 'sql');
        INSERT INTO Item VALUES (1, 'sword'), (1, 'shield'), (2, 'potion');
        ",
    )
    .await;

    let expected = json!([
        {
            "type": "SELECT",
            "rows": [
                { "name": "glue", "title": "shield" },
                { "name": "glue", "title": "sword" },
                { "name": "sql", "title": "potion" },
            ],
        },
    ]);
    assert_eq!(
        run(
            &mut glue,
            "
            SELECT Player.name AS name, Item.title AS title
            FROM Player
            JOIN Item ON Player.id = Item.owner_id
            ORDER BY name, title;
            ",
        )
        .await,
        expected
    );
}

#[wasm_bindgen_test]
async fn aggregate() {
    let mut glue = open("test-aggregate").await;

    run(
        &mut glue,
        "
        CREATE TABLE Sale (category TEXT, amount INTEGER);
        INSERT INTO Sale VALUES
            ('fruit', 100), ('fruit', 250), ('dairy', 80), ('dairy', 20), ('fruit', 50);
        ",
    )
    .await;

    let expected = json!([
        {
            "type": "SELECT",
            "rows": [
                { "category": "dairy", "cnt": 2, "total": 100 },
                { "category": "fruit", "cnt": 3, "total": 400 },
            ],
        },
    ]);
    assert_eq!(
        run(
            &mut glue,
            "
            SELECT category, COUNT(*) AS cnt, SUM(amount) AS total
            FROM Sale
            GROUP BY category
            ORDER BY category;
            ",
        )
        .await,
        expected
    );
}

#[wasm_bindgen_test]
async fn update() {
    let mut glue = open("test-update").await;

    run(
        &mut glue,
        "
        CREATE TABLE Foo (id INTEGER, name TEXT);
        INSERT INTO Foo VALUES (1, 'before'), (2, 'stay');
        UPDATE Foo SET name = 'after' WHERE id = 1;
        ",
    )
    .await;

    let expected = json!([
        {
            "type": "SELECT",
            "rows": [
                { "id": 1, "name": "after" },
                { "id": 2, "name": "stay" },
            ],
        },
    ]);
    assert_eq!(
        run(&mut glue, "SELECT * FROM Foo ORDER BY id").await,
        expected
    );
}

#[wasm_bindgen_test]
async fn persists_across_reopen() {
    let mut glue = open("test-persistence").await;

    run(
        &mut glue,
        "
        CREATE TABLE Foo (id INTEGER, name TEXT);
        INSERT INTO Foo VALUES (1, 'persisted'), (2, 'data');
        DELETE FROM Foo WHERE id = 2;
        ",
    )
    .await;

    drop(glue);

    let mut glue = open("test-persistence").await;

    let expected = json!([
        {
            "type": "SELECT",
            "rows": [{ "id": 1, "name": "persisted" }],
        },
    ]);
    assert_eq!(
        run(&mut glue, "SELECT * FROM Foo ORDER BY id").await,
        expected
    );
}
