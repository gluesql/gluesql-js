//! Logical export and import. `dump` writes every table as SQL text, and the
//! `restore_*` functions load such text back so that a failed restore leaves
//! the database as it was.

use {
    gluesql_core::{
        ast::ToSql,
        data::{Schema, Value},
        error::Error as CoreError,
        plan::StatementPlan,
        prelude::{Glue, parse},
        store::{GStore, GStoreMut, Planner},
    },
    std::fmt,
};

const ROWS_PER_INSERT: usize = 100;

/// Message shown to the caller; the bindings only turn errors into strings.
#[derive(Debug)]
pub struct Error(String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<CoreError> for Error {
    fn from(error: CoreError) -> Self {
        Self(error.to_string())
    }
}

type Result<T> = std::result::Result<T, Error>;

/// Returns the rows of every table as batched `INSERT` statements, each table
/// preceded by its `CREATE TABLE` when `with_schema` is set. Foreign key
/// targets come ahead of the tables that reference them.
pub fn dump<T: GStore + GStoreMut>(storage: &mut T, with_schema: bool) -> Result<String> {
    // `RedbStorage` reads schemas through a transaction. `begin` reports
    // whether it opened one, so a transaction the caller left open is reused
    // and stays open.
    let opened = storage.begin(true)?;
    let sql = tables_sql(storage, with_schema);

    if opened {
        storage.rollback()?;
    }

    sql
}

fn tables_sql<T: GStore>(storage: &T, with_schema: bool) -> Result<String> {
    let mut sql = String::new();

    for schema in order_by_foreign_keys(storage.fetch_all_schemas()?) {
        if with_schema {
            sql.push_str(&create_table_sql(&schema)?);
            sql.push('\n');
        }

        let insert_into = insert_into_sql(&schema);
        let mut rows = Vec::with_capacity(ROWS_PER_INSERT);

        for row in storage.scan_data(&schema.table_name)? {
            let (_, values) = row?;
            let values = values.iter().map(value_sql).collect::<Vec<_>>().join(", ");

            rows.push(format!("({values})"));

            if rows.len() == ROWS_PER_INSERT {
                push_insert(&mut sql, &insert_into, &mut rows);
            }
        }

        push_insert(&mut sql, &insert_into, &mut rows);
    }

    Ok(sql)
}

/// `INSERT INTO "table" ("column", ...) VALUES `. Naming the columns lets the
/// rows load into a table that keeps them in another order or has more of
/// them. A schemaless table has no columns to name.
fn insert_into_sql(schema: &Schema) -> String {
    let table_name = quoted(&schema.table_name);

    match &schema.column_defs {
        Some(column_defs) => {
            let columns = column_defs
                .iter()
                .map(|column_def| quoted(&column_def.name))
                .collect::<Vec<_>>()
                .join(", ");

            format!("INSERT INTO {table_name} ({columns}) VALUES ")
        }
        None => format!("INSERT INTO {table_name} VALUES "),
    }
}

fn quoted(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

fn push_insert(sql: &mut String, insert_into: &str, rows: &mut Vec<String>) {
    if rows.is_empty() {
        return;
    }

    sql.push_str(insert_into);
    sql.push_str(&rows.join(", "));
    sql.push_str(";\n");
    rows.clear();
}

/// Sorts by name for a stable output, then moves every table behind the
/// tables its foreign keys reference.
fn order_by_foreign_keys(mut schemas: Vec<Schema>) -> Vec<Schema> {
    schemas.sort_by(|a, b| a.table_name.cmp(&b.table_name));

    let mut ordered: Vec<Schema> = Vec::with_capacity(schemas.len());

    while !schemas.is_empty() {
        let (ready, waiting): (Vec<_>, Vec<_>) = schemas.into_iter().partition(|schema| {
            schema.foreign_keys.iter().all(|foreign_key| {
                let referenced = &foreign_key.referenced_table_name;

                referenced == &schema.table_name
                    || ordered.iter().any(|done| &done.table_name == referenced)
            })
        });

        if ready.is_empty() {
            // Nothing left can be placed (a reference cycle or a missing
            // target); keep the rest in name order instead of looping.
            ordered.extend(waiting);
            break;
        }

        ordered.extend(ready);
        schemas = waiting;
    }

    ordered
}

/// `Schema::to_ddl` with the quotes inside `COMMENT` strings escaped, which
/// it leaves as they are. Identifiers containing a double quote are not
/// escaped by it either, so the statement is parsed once here: a dump that
/// cannot be restored fails now instead of at restore time.
fn create_table_sql(schema: &Schema) -> Result<String> {
    let escape = |comment: &mut Option<String>| {
        if let Some(text) = comment {
            *text = text.replace('\'', "''");
        }
    };

    let mut escaped = schema.clone();
    escape(&mut escaped.comment);
    for column_def in escaped.column_defs.iter_mut().flatten() {
        escape(&mut column_def.comment);
    }

    let ddl = escaped.to_ddl();

    match parse(&ddl) {
        Ok(_) => Ok(ddl),
        Err(cause) => Err(Error(format!(
            "dump: table {} cannot be written as SQL that parses back: {cause}",
            schema.table_name
        ))),
    }
}

/// `Value::to_sql` with the forms that do not parse back replaced: NaN, the
/// infinities and negative zero in floats, and `POINT(x y)` without a comma.
fn value_sql(value: &Value) -> String {
    match value {
        Value::F32(n) if needs_cast(f64::from(*n)) => format!("CAST('{n}' AS FLOAT32)"),
        Value::F64(n) => float_sql(*n),
        Value::Point(point) => format!("POINT({}, {})", float_sql(point.x), float_sql(point.y)),
        _ => value.to_sql(),
    }
}

fn float_sql(n: f64) -> String {
    if needs_cast(n) {
        format!("CAST('{n}' AS FLOAT)")
    } else {
        n.to_string()
    }
}

fn needs_cast(n: f64) -> bool {
    !n.is_finite() || (n == 0.0 && n.is_sign_negative())
}

/// Plans the whole text first, so a syntax error is reported before any
/// statement runs.
fn plan<T: GStore + GStoreMut + Planner>(
    glue: &mut Glue<T>,
    sql: &str,
) -> Result<Vec<StatementPlan>> {
    let statements = glue.plan(sql)?;

    if writes_only_to_created_tables(&statements) {
        Ok(statements)
    } else {
        Err(Error(
            "restore: only CREATE TABLE without IF NOT EXISTS, and INSERT or CREATE INDEX \
             on a table created by the same text, are accepted"
                .to_owned(),
        ))
    }
}

/// The shape `dump` writes. A restore held to it changes nothing but the
/// tables it creates, so dropping those tables undoes it completely: storages
/// without transactions end up exactly where a rollback would leave them.
fn writes_only_to_created_tables(statements: &[StatementPlan]) -> bool {
    let mut created = Vec::new();

    statements.iter().all(|statement| match statement {
        StatementPlan::CreateTable {
            name,
            if_not_exists: false,
            ..
        } => {
            created.push(name);

            true
        }
        StatementPlan::Insert { table_name, .. }
        | StatementPlan::CreateIndex { table_name, .. } => created.contains(&table_name),
        _ => false,
    })
}

/// Runs `sql` in one transaction and rolls it back on failure. For storages
/// with transactions.
#[cfg(all(target_arch = "wasm32", feature = "opfs"))]
pub fn restore_in_transaction<T: GStore + GStoreMut + Planner>(
    glue: &mut Glue<T>,
    sql: &str,
) -> Result<()> {
    let statements = plan(glue, sql)?;

    // Fails when the caller already has a transaction open, and leaves that
    // transaction as it is.
    glue.storage.begin(false)?;

    let executed = statements
        .iter()
        .try_for_each(|statement| glue.execute_stmt(statement).map(|_| ()));

    match executed {
        Ok(()) => Ok(glue.storage.commit()?),
        Err(error) => {
            glue.storage.rollback()?;

            Err(error.into())
        }
    }
}

/// Runs `sql` and, on failure, drops the tables it created. For storages
/// without transactions.
#[cfg(not(all(target_arch = "wasm32", feature = "opfs")))]
pub fn restore_or_drop_created<T: GStore + GStoreMut + Planner>(
    glue: &mut Glue<T>,
    sql: &str,
) -> Result<()> {
    let statements = plan(glue, sql)?;
    let mut created = Vec::new();
    let executed = execute_recording_created(glue, &statements, &mut created);

    if executed.is_err() {
        // `delete_schema` skips the foreign key checks of `DROP TABLE`;
        // referencing tables still go first.
        for table_name in created.iter().rev() {
            glue.storage.delete_schema(table_name)?;
        }
    }

    executed
}

#[cfg(not(all(target_arch = "wasm32", feature = "opfs")))]
fn execute_recording_created<'a, T: GStore + GStoreMut + Planner>(
    glue: &mut Glue<T>,
    statements: &'a [StatementPlan],
    created: &mut Vec<&'a str>,
) -> Result<()> {
    for statement in statements {
        // `CREATE TABLE` fails for a name that is taken, and that table must
        // survive, so a name is recorded only while it is still free.
        if let StatementPlan::CreateTable { name, .. } = statement
            && glue.storage.fetch_schema(name)?.is_none()
        {
            created.push(name);
        }

        glue.execute_stmt(statement)?;
    }

    Ok(())
}
