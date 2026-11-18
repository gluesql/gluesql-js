const assert = require('node:assert/strict');
const { test } = require('node:test');
const { gluesql } = require('../../gluesql.node.js');
const { tempPath } = require('./support.js');

async function withDatabase(t, run) {
  const file = tempPath(t, 'transactions.db');
  let db = gluesql({ storage: 'redb', path: file });

  try {
    await run(db, () => {
      db.close();
      db = gluesql({ storage: 'redb', path: file });
      return db;
    });
  } finally {
    db.close();
  }
}

async function rows(db, sql = 'SELECT * FROM User ORDER BY id') {
  return (await db.query(sql))[0].rows;
}

test('explicit COMMIT persists changes across query calls and reopening', async (t) => {
  await withDatabase(t, async (db, reopen) => {
    await db.query('BEGIN');
    await db.query('CREATE TABLE User (id INTEGER PRIMARY KEY, name TEXT)');
    await db.query("INSERT INTO User VALUES (1, 'glue')");
    assert.deepEqual(await rows(db), [{ id: 1, name: 'glue' }]);
    await db.query("UPDATE User SET name = 'sql' WHERE id = 1");
    await db.query('COMMIT');

    db = reopen();
    assert.deepEqual(await rows(db), [{ id: 1, name: 'sql' }]);
  });
});

test('ROLLBACK restores rows and schemas, including after reopening', async (t) => {
  await withDatabase(t, async (db, reopen) => {
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

    db = reopen();
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
  await withDatabase(t, async (db, reopen) => {
    await db.query('CREATE TABLE User (id INTEGER PRIMARY KEY)');
    await db.query('BEGIN; INSERT INTO User VALUES (1)');
    await assert.rejects(() => db.query('BEGIN'), /nested transaction/i);
    await assert.rejects(() => db.query('INSERT INTO User VALUES (1)'), /duplicate|unique/i);
    assert.deepEqual(await rows(db), [{ id: 1 }]);
    await db.query('INSERT INTO User VALUES (2)');
    await db.query('ROLLBACK');

    db = reopen();
    assert.deepEqual(await rows(db), []);
    await db.query('BEGIN; INSERT INTO User VALUES (3); COMMIT');
    assert.deepEqual(await rows(db), [{ id: 3 }]);
  });
});

test('a failed restore-shaped batch rolls back every newly created table', async (t) => {
  await withDatabase(t, async (db, reopen) => {
    await db.query('CREATE TABLE Kept (id INTEGER); INSERT INTO Kept VALUES (7)');
    await db.query('BEGIN');
    await assert.rejects(() => db.query(`
      CREATE TABLE First (id INTEGER PRIMARY KEY);
      INSERT INTO First VALUES (1);
      CREATE TABLE Second (id INTEGER);
      INSERT INTO First VALUES (1);
    `), /duplicate|unique/i);
    await db.query('ROLLBACK');

    db = reopen();
    assert.deepEqual(await db.query('SHOW TABLES'), [
      { type: 'SHOW TABLES', tables: ['Kept'] },
    ]);
    assert.deepEqual(await rows(db, 'SELECT * FROM Kept'), [{ id: 7 }]);
  });
});

test('closing direct redb releases the file and discards uncommitted changes', async (t) => {
  const path = tempPath(t, 'close.db');
  const db = gluesql({ storage: 'redb', path });

  try {
    await db.query('CREATE TABLE User (id INTEGER)');
    await db.query('BEGIN; INSERT INTO User VALUES (1)');
    assert.throws(() => gluesql({ storage: 'redb', path }), /already open|lock/i);
    db.close();
    db.close();
    await assert.rejects(() => db.query('SELECT * FROM User'), /closed/);

    const reopened = gluesql({ storage: 'redb', path });
    try {
      assert.deepEqual(await rows(reopened), []);
    } finally {
      reopened.close();
    }
  } finally {
    db.close();
  }
});

test('direct redb rejects registry options and invalid storage config', (t) => {
  const path = tempPath(t, 'invalid.db');
  for (const options of [{ engines: {} }, { defaultEngine: 'local' }, { paht: path }]) {
    assert.throws(() => gluesql({ storage: 'redb', path, ...options }), /unknown field/);
  }
  assert.throws(() => gluesql({ storage: 'redb' }), /missing field `path`/);
  assert.throws(() => gluesql({ storage: 'memory' }), /direct storage must be redb/);
});

test('the engine registry rejects explicit transactions even with a redb default', async (t) => {
  const path = tempPath(t, 'registry.db');
  const db = gluesql({ engines: { local: { storage: 'redb', path } } });

  try {
    for (const engine of ['memory', 'local']) {
      db.setDefaultEngine(engine);
      await assert.rejects(() => db.query('BEGIN'), /explicit transactions are not supported/);
    }

    await db.query('CREATE TABLE User (id INTEGER); INSERT INTO User VALUES (1)');
    assert.deepEqual(await rows(db), [{ id: 1 }]);
  } finally {
    db.setDefaultEngine('memory');
    db.removeEngine('local');
  }
});
