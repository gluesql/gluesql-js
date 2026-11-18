const assert = require('node:assert/strict');
const { test } = require('node:test');
const { gluesql } = require('../../gluesql.node.js');
const dumpCases = require('../dump-cases.js');

const tableNames = async (db) => (await db.query('SHOW TABLES'))[0].tables;

const allRows = async (db) => {
  const rows = {};

  for (const table of await tableNames(db)) {
    [{ rows: rows[table] }] = await db.query(`SELECT * FROM "${table}"`);
  }

  return rows;
};

test('dump() writes CREATE TABLE and batched INSERT statements', async () => {
  const db = gluesql();

  assert.equal(await db.dump(), '');

  await db.query(`
    CREATE TABLE Foo (id INTEGER, name TEXT);
    CREATE TABLE Empty (id INTEGER);
    INSERT INTO Foo VALUES (1, 'glue'), (2, 'it''s');
  `);

  // gluesql-core puts two spaces ahead of ENGINE; the test does not pin that.
  assert.equal(
    (await db.dump()).replaceAll('  ENGINE', ' ENGINE'),
    [
      'CREATE TABLE "Empty" ("id" INT NULL) ENGINE = memory;',
      'CREATE TABLE "Foo" ("id" INT NULL, "name" TEXT NULL) ENGINE = memory;',
      `INSERT INTO "Foo" ("id", "name") VALUES (1, 'glue'), (2, 'it''s');`,
      '',
    ].join('\n'),
  );
  assert.equal(await db.dump({ schema: true }), await db.dump());
  assert.equal(
    await db.dump({ schema: false }),
    `INSERT INTO "Foo" ("id", "name") VALUES (1, 'glue'), (2, 'it''s');\n`,
  );

  // Forms that gluesql-core prints in a way its own parser rejects.
  await db.query(`
    CREATE TABLE Special (f FLOAT, p POINT) COMMENT = 'it''s';
    INSERT INTO Special VALUES
      (CAST('NaN' AS FLOAT), POINT(1.5, -2.25)),
      (CAST('-inf' AS FLOAT), NULL),
      (CAST('-0.0' AS FLOAT), NULL);
  `);

  const special = await db.dump();

  for (const expected of [
    "COMMENT = 'it''s'",
    "(CAST('NaN' AS FLOAT), POINT(1.5, -2.25))",
    "(CAST('-inf' AS FLOAT), NULL)",
    "(CAST('-0' AS FLOAT), NULL)",
  ]) {
    assert.ok(special.includes(expected), `${expected} not in ${special}`);
  }

  // Referenced tables come first, whatever the names sort to.
  await db.query(`
    CREATE TABLE ZParent (id INTEGER PRIMARY KEY);
    CREATE TABLE AChild (pid INTEGER, FOREIGN KEY (pid) REFERENCES ZParent (id));
  `);

  const ordered = await db.dump();

  assert.ok(
    ordered.indexOf('CREATE TABLE "ZParent"') <
      ordered.indexOf('CREATE TABLE "AChild"'),
  );
});

test('restore() rebuilds what dump() wrote, for every value type', async (t) => {
  for (const { name, sql } of dumpCases) {
    await t.test(name, async () => {
      const source = gluesql();
      await source.query(sql);

      const dumped = await source.dump();

      const target = gluesql();
      await target.restore(dumped);

      assert.equal(await target.dump(), dumped);
      assert.deepEqual(await allRows(target), await allRows(source));
    });
  }
});

test('a failed restore() leaves the database as it was', async () => {
  const db = gluesql();

  await db.query(`
    CREATE TABLE Keep (id INTEGER);
    INSERT INTO Keep VALUES (1);
  `);

  const unchanged = async () => {
    assert.deepEqual(await tableNames(db), ['Keep']);
    assert.deepEqual(await allRows(db), { Keep: [{ id: 1 }] });
  };

  // A statement fails after tables were created: they are dropped again,
  // the referencing table included.
  await assert.rejects(
    () =>
      db.restore(`
        CREATE TABLE Parent (id INTEGER PRIMARY KEY);
        CREATE TABLE Child (pid INTEGER, FOREIGN KEY (pid) REFERENCES Parent (id));
        INSERT INTO Parent VALUES (1);
        INSERT INTO Child VALUES (1);
        INSERT INTO Child VALUES (99);
      `),
    /cannot find referenced value on Parent.id with value "99"/,
  );
  await unchanged();

  // An existing table is neither replaced nor dropped.
  await assert.rejects(
    () => db.restore('CREATE TABLE Keep (id INTEGER); INSERT INTO Keep VALUES (2);'),
    /table already exists: Keep/,
  );
  await unchanged();

  // A syntax error is reported before anything runs.
  await assert.rejects(
    () => db.restore('CREATE TABLE Other (id INTEGER); INSERT INTO Other VALUE (1);'),
    /parser/,
  );
  await unchanged();

  // Only what dump() writes is accepted: new tables and rows for them.
  for (const sql of [
    'INSERT INTO Keep VALUES (2);',
    'CREATE TABLE IF NOT EXISTS Keep (id INTEGER);',
    'INSERT INTO Other VALUES (1); CREATE TABLE Other (id INTEGER);',
    'DROP TABLE Keep;',
    'DELETE FROM Keep;',
    'CREATE TABLE Other (id INTEGER); COMMIT;',
  ]) {
    await assert.rejects(
      () => db.restore(sql),
      /restore: only CREATE TABLE without IF NOT EXISTS, and INSERT or CREATE INDEX on a table created by the same text, are accepted/,
      sql,
    );
    await unchanged();
  }
});

// Rows only, for a database that already has the tables: columns are named,
// so they may be in another order there, next to columns and rows of its own.
test('dump({ schema: false }) writes rows for tables that already exist', async () => {
  const source = gluesql();

  await source.query(`
    CREATE TABLE Note (id INTEGER, body TEXT);
    CREATE TABLE Doc;
    INSERT INTO Note VALUES (1, 'local'), (2, 'it''s');
    INSERT INTO Doc VALUES ('{"a": 1}');
  `);

  const data = await source.dump({ schema: false });

  assert.equal(
    data,
    [
      `INSERT INTO "Doc" VALUES ('{"a":1}');`,
      `INSERT INTO "Note" ("id", "body") VALUES (1, 'local'), (2, 'it''s');`,
      '',
    ].join('\n'),
  );

  const target = gluesql();

  await target.query(`
    CREATE TABLE Note (body TEXT, owner TEXT DEFAULT 'server', id INTEGER);
    CREATE TABLE Doc;
    INSERT INTO Note VALUES ('existing', 'someone', 100);
  `);
  await target.query(data);

  assert.deepEqual(
    (await target.query('SELECT id, body, owner FROM Note ORDER BY id'))[0].rows,
    [
      { id: 1, body: 'local', owner: 'server' },
      { id: 2, body: "it's", owner: 'server' },
      { id: 100, body: 'existing', owner: 'someone' },
    ],
  );
  assert.deepEqual((await target.query('SELECT * FROM Doc'))[0].rows, [{ a: 1 }]);

  // restore() only adds tables, so this text goes through query().
  await assert.rejects(() => target.restore(data), /restore: only CREATE TABLE/);
});

test('dump() rejects a table it cannot write as SQL', async () => {
  const db = gluesql();

  await db.query('CREATE TABLE "a""b" ("c""d" INTEGER); INSERT INTO "a""b" VALUES (1);');

  await assert.rejects(
    () => db.dump(),
    /dump: table a"b cannot be written as SQL that parses back/,
  );
  assert.equal(
    await db.dump({ schema: false }),
    'INSERT INTO "a""b" ("c""d") VALUES (1);\n',
  );
});

test('dump() names the engine of each table and restore() needs that engine', async () => {
  const engines = { scratch: { storage: 'memory' } };
  const source = gluesql({ engines });

  await source.query(`
    CREATE TABLE Main (id INTEGER);
    CREATE TABLE Scratch (id INTEGER) ENGINE = scratch;
    INSERT INTO Main VALUES (1);
    INSERT INTO Scratch VALUES (2);
  `);

  const dumped = await source.dump();

  assert.match(dumped, /CREATE TABLE "Main" .* ENGINE = memory;/);
  assert.match(dumped, /CREATE TABLE "Scratch" .* ENGINE = scratch;/);

  const target = gluesql({ engines });
  await target.restore(dumped);

  assert.equal(await target.dump(), dumped);

  // Without the engine the restore fails as a whole: `Main` is dropped again.
  const plain = gluesql();

  await assert.rejects(
    () => plain.restore(dumped),
    /storage not found for table: Scratch/,
  );
  assert.deepEqual(await tableNames(plain), []);
});
