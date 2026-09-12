const assert = require('node:assert/strict');
const { test } = require('node:test');
const { gluesql } = require('../../gluesql.node.js');

test('executes multiple statements in one query', async () => {
  const db = gluesql();

  assert.deepEqual(
    await db.query(`
      CREATE TABLE Foo (id INTEGER, name TEXT);
      INSERT INTO Foo VALUES (1, 'glue'), (2, 'sql');
    `),
    [
      { type: 'CREATE TABLE' },
      { type: 'INSERT', affected: 2 },
    ],
  );

  assert.deepEqual(await db.query('SELECT * FROM Foo ORDER BY id'), [
    {
      type: 'SELECT',
      rows: [
        { id: 1, name: 'glue' },
        { id: 2, name: 'sql' },
      ],
    },
  ]);
});

test('rejects with the storage error message', async () => {
  const db = gluesql();

  await assert.rejects(
    () => db.query('SELECT * FROM Missing'),
    /fetch: table not found: Missing/,
  );
});
