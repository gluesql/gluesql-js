const assert = require('node:assert/strict');
const { test } = require('node:test');
const { gluesql } = require('../../gluesql.node.js');
const { tempPath } = require('./support.js');

async function withDatabase(t, run) {
  const file = tempPath(t, 'transactions.db');
  const db = gluesql({
    engines: { local: { storage: 'redb', path: file } },
    defaultEngine: 'local',
  });

  try {
    await run(db, file);
  } finally {
    await db.query('ROLLBACK');
    db.setDefaultEngine('memory');
    for (const engine of db.listEngines()) {
      if (engine !== 'memory') db.removeEngine(engine);
    }
  }
}

function reopen(db, file) {
  db.setDefaultEngine('memory');
  db.removeEngine('local');
  db.addEngine('local', { storage: 'redb', path: file });
  db.setDefaultEngine('local');
}

async function rows(db, sql = 'SELECT * FROM User ORDER BY id') {
  return (await db.query(sql))[0].rows;
}

test('explicit COMMIT persists changes across query calls and reopening', async (t) => {
  await withDatabase(t, async (db, file) => {
    await db.query('BEGIN');
    await db.query('CREATE TABLE User (id INTEGER PRIMARY KEY, name TEXT)');
    await db.query("INSERT INTO User VALUES (1, 'glue')");
    assert.deepEqual(await rows(db), [{ id: 1, name: 'glue' }]);
    await db.query("UPDATE User SET name = 'sql' WHERE id = 1");
    await db.query('COMMIT');

    reopen(db, file);
    assert.deepEqual(await rows(db), [{ id: 1, name: 'sql' }]);
  });
});

test('ROLLBACK restores rows and schemas, including after reopening', async (t) => {
  await withDatabase(t, async (db, file) => {
    await db.query(`
      CREATE TABLE User (id INTEGER PRIMARY KEY, name TEXT);
      INSERT INTO User VALUES (1, 'original'), (2, 'kept');
      CREATE TABLE Kept (id INTEGER);
    `);
    await db.query('BEGIN');
    await db.query("UPDATE User SET name = 'changed' WHERE id = 1");
    await db.query('DELETE FROM User WHERE id = 2');
    await db.query("INSERT INTO User VALUES (3, 'new')");
    await db.query('DROP TABLE Kept; CREATE TABLE NewTable (id INTEGER)');
    await db.query('ALTER TABLE User ADD COLUMN extra TEXT');
    await db.query('ROLLBACK');

    reopen(db, file);
    assert.deepEqual(await rows(db), [
      { id: 1, name: 'original' },
      { id: 2, name: 'kept' },
    ]);
    assert.deepEqual(await db.query('SHOW TABLES'), [
      { type: 'SHOW TABLES', tables: ['Kept', 'User'] },
    ]);
  });
});

test('failed statements and nested BEGIN leave the original transaction open', async (t) => {
  await withDatabase(t, async (db, file) => {
    await db.query('CREATE TABLE User (id INTEGER PRIMARY KEY)');
    await db.query('BEGIN; INSERT INTO User VALUES (1)');
    await assert.rejects(() => db.query('BEGIN'), /nested transaction/i);
    await assert.rejects(() => db.query('INSERT INTO User VALUES (1)'), /duplicate|unique/i);
    assert.deepEqual(await rows(db), [{ id: 1 }]);
    await db.query('INSERT INTO User VALUES (2)');
    await db.query('ROLLBACK');

    reopen(db, file);
    assert.deepEqual(await rows(db), []);
    await db.query('BEGIN; INSERT INTO User VALUES (3); COMMIT');
    assert.deepEqual(await rows(db), [{ id: 3 }]);
  });
});

test('a failed restore-shaped batch rolls back every newly created table', async (t) => {
  await withDatabase(t, async (db, file) => {
    await db.query('CREATE TABLE Kept (id INTEGER); INSERT INTO Kept VALUES (7)');
    await db.query('BEGIN');
    await assert.rejects(() => db.query(`
      CREATE TABLE First (id INTEGER PRIMARY KEY);
      INSERT INTO First VALUES (1);
      CREATE TABLE Second (id INTEGER);
      INSERT INTO First VALUES (1);
    `), /duplicate|unique/i);
    await db.query('ROLLBACK');

    reopen(db, file);
    assert.deepEqual(await db.query('SHOW TABLES'), [
      { type: 'SHOW TABLES', tables: ['Kept'] },
    ]);
    assert.deepEqual(await rows(db, 'SELECT * FROM Kept'), [{ id: 7 }]);
  });
});

test('transaction reads span engines but writes are confined to its pinned redb', async (t) => {
  await withDatabase(t, async (db, file) => {
    db.addEngine('other', { storage: 'redb', path: tempPath(t, 'other.db') });
    await db.query(`
      CREATE TABLE User (id INTEGER) ENGINE = local;
      CREATE TABLE Cached (id INTEGER) ENGINE = memory;
      CREATE TABLE Stored (id INTEGER) ENGINE = other;
      INSERT INTO Cached VALUES (1);
      INSERT INTO Stored VALUES (1);
    `);
    await db.query('BEGIN; INSERT INTO User VALUES (1)');
    assert.deepEqual(await rows(db, `
      SELECT U.id FROM User AS U
      JOIN Cached ON U.id = Cached.id
      JOIN Stored ON U.id = Stored.id
    `), [{ id: 1 }]);
    assert.deepEqual(await db.query('SHOW TABLES'), [
      { type: 'SHOW TABLES', tables: ['Cached', 'Stored', 'User'] },
    ]);

    for (const table of ['Cached', 'Stored']) {
      for (const sql of [
        `INSERT INTO ${table} VALUES (2)`,
        `UPDATE ${table} SET id = 2`,
        `DELETE FROM ${table}`,
        `DROP TABLE ${table}`,
        `ALTER TABLE ${table} ADD COLUMN extra TEXT`,
      ]) {
        await assert.rejects(() => db.query(sql), /cannot write to engine/);
      }
    }
    for (const engine of ['memory', 'other']) {
      await assert.rejects(
        () => db.query(`CREATE TABLE Forbidden (id INTEGER) ENGINE = ${engine}`),
        /cannot write to engine/,
      );
    }
    await db.query('COMMIT');
    assert.deepEqual(await rows(db, 'SELECT * FROM Cached'), [{ id: 1 }]);
    assert.deepEqual(await rows(db, 'SELECT * FROM Stored'), [{ id: 1 }]);
    reopen(db, file);
    assert.deepEqual(await rows(db), [{ id: 1 }]);

    db.setDefaultEngine('other');
    await db.query('BEGIN; INSERT INTO Stored VALUES (2); COMMIT');
    assert.deepEqual(await rows(db, 'SELECT * FROM Stored ORDER BY id'), [{ id: 1 }, { id: 2 }]);
  });
});

test('engine configuration and registry functions cannot escape a transaction', async (t) => {
  await withDatabase(t, async (db) => {
    await db.query('CREATE FUNCTION add_one (n INT) RETURN n + 1');
    await db.query('BEGIN; CREATE TABLE User (id INTEGER)');
    assert.throws(() => db.addEngine('scratch', { storage: 'memory' }), /during a transaction/);
    assert.throws(() => db.removeEngine('memory'), /during a transaction/);
    assert.throws(() => db.setDefaultEngine('memory'), /during a transaction/);
    await assert.rejects(() => db.query('DROP FUNCTION add_one'), /during a transaction/);
    await assert.rejects(
      () => db.query('CREATE FUNCTION add_two (n INT) RETURN n + 2'),
      /during a transaction/,
    );
    assert.deepEqual(await rows(db, 'SELECT add_one(1) AS value'), [{ value: 2 }]);
    await db.query('ROLLBACK');
    assert.equal(db.defaultEngine(), 'local');
    assert.deepEqual(db.listEngines(), ['local', 'memory']);
    assert.deepEqual(await db.query('SHOW TABLES'), [{ type: 'SHOW TABLES', tables: [] }]);
    await db.query('DROP FUNCTION add_one');
  });
});

test('BEGIN uses the default backend and leaves memory non-transactional', async (t) => {
  await withDatabase(t, async (db) => {
    db.setDefaultEngine('memory');
    await assert.rejects(() => db.query('BEGIN'), /MemoryStorage.*transaction is not supported/);
    await db.query('CREATE TABLE Cached (id INTEGER); INSERT INTO Cached VALUES (1)');
    db.setDefaultEngine('local');
    await db.query('BEGIN; CREATE TABLE User (id INTEGER); ROLLBACK');
    assert.deepEqual(await rows(db, 'SELECT * FROM Cached'), [{ id: 1 }]);
    await assert.rejects(() => db.query('SELECT * FROM User'), /table not found/);
  });
});
