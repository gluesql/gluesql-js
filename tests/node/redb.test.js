const assert = require('node:assert/strict');
const { test } = require('node:test');
const { gluesql } = require('../../gluesql.node.js');
const { tempPath } = require('./support.js');

function open(file) {
  return gluesql({
    engines: { local: { storage: 'redb', path: file } },
    defaultEngine: 'local',
  });
}

/// Releases the exclusive lock the redb engine holds on its file.
function close(db) {
  db.setDefaultEngine('memory');
  db.removeEngine('local');
}

test('keeps data on disk across instances', async (t) => {
  const file = tempPath(t, 'redb.db');

  const writer = open(file);
  await writer.query(`
    CREATE TABLE User (id INTEGER, name TEXT);
    INSERT INTO User VALUES (1, 'glue');
  `);
  close(writer);

  const reader = open(file);

  assert.deepEqual(await reader.query('SELECT * FROM User'), [
    { type: 'SELECT', rows: [{ id: 1, name: 'glue' }] },
  ]);
});

test('persists appends, updates, deletes and drops', async (t) => {
  const file = tempPath(t, 'mutations.db');

  const first = open(file);
  await first.query(`
    CREATE TABLE User (id INTEGER, name TEXT);
    INSERT INTO User VALUES (1, 'glue');
  `);
  await first.query("INSERT INTO User VALUES (2, 'sql'), (3, 'sticky')");
  close(first);

  const second = open(file);
  assert.deepEqual(await second.query('SELECT * FROM User ORDER BY id'), [
    {
      type: 'SELECT',
      rows: [
        { id: 1, name: 'glue' },
        { id: 2, name: 'sql' },
        { id: 3, name: 'sticky' },
      ],
    },
  ]);

  assert.deepEqual(await second.query("UPDATE User SET name = 'glued' WHERE id = 1"), [
    { type: 'UPDATE', affected: 1 },
  ]);
  assert.deepEqual(await second.query('DELETE FROM User WHERE id = 3'), [
    { type: 'DELETE', affected: 1 },
  ]);
  close(second);

  const third = open(file);
  assert.deepEqual(await third.query('SELECT * FROM User ORDER BY id'), [
    {
      type: 'SELECT',
      rows: [
        { id: 1, name: 'glued' },
        { id: 2, name: 'sql' },
      ],
    },
  ]);

  await third.query('DROP TABLE User');
  close(third);

  const fourth = open(file);
  await assert.rejects(
    () => fourth.query('SELECT * FROM User'),
    /table not found: User/,
  );
});

test('joins a redb table with an in-memory table', async (t) => {
  const file = tempPath(t, 'join.db');
  const db = gluesql({ engines: { local: { storage: 'redb', path: file } } });

  await db.query(`
    CREATE TABLE Cached (id INTEGER) ENGINE = memory;
    CREATE TABLE Stored (id INTEGER, name TEXT) ENGINE = local;
    INSERT INTO Cached VALUES (1);
    INSERT INTO Stored VALUES (1, 'glue');
  `);

  assert.deepEqual(
    await db.query(
      'SELECT Stored.name FROM Cached JOIN Stored ON Cached.id = Stored.id',
    ),
    [{ type: 'SELECT', rows: [{ name: 'glue' }] }],
  );

  // `Stored` really went to redb: it survives in a new instance, `Cached` does not.
  db.removeEngine('local');
  const reopened = open(file);

  assert.deepEqual(await reopened.query('SELECT * FROM Stored'), [
    { type: 'SELECT', rows: [{ id: 1, name: 'glue' }] },
  ]);
  await assert.rejects(
    () => reopened.query('SELECT * FROM Cached'),
    /table not found: Cached/,
  );
});

test('reports the lock held on an open database file', async (t) => {
  const file = tempPath(t, 'locked.db');
  const holder = open(file);
  await holder.query('CREATE TABLE User (id INTEGER)');

  assert.throws(() => open(file), /already open|lock/i);

  close(holder);

  assert.deepEqual(await open(file).query('SELECT * FROM User'), [
    { type: 'SELECT', rows: [] },
  ]);
});

test('reports the failure when the path is not usable', (t) => {
  const db = gluesql();

  assert.throws(
    () => db.addEngine('local', { storage: 'redb', path: tempPath(t, 'missing/nested.db') }),
    /storage: I\/O error/,
  );
});

test('requires the path option', () => {
  const db = gluesql();

  assert.throws(
    () => db.addEngine('local', { storage: 'redb' }),
    /invalid storage config: missing field `path`/,
  );
});
