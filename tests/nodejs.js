const assert = require('node:assert/strict');
const { gluesql } = require('../gluesql.node.js');

async function main() {
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

  await assert.rejects(
    () => db.query('SELECT * FROM Missing'),
    /fetch: table not found: Missing/,
  );
}

main();
