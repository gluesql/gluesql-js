const assert = require('node:assert/strict');
const { test } = require('node:test');
const { gluesql, storages } = require('../../gluesql.node.js');

test('starts with the in-memory engine as default', () => {
  const db = gluesql();

  assert.deepEqual(db.listEngines(), ['memory']);
  assert.equal(db.defaultEngine(), 'memory');
});

test('registers engines declaratively', () => {
  const db = gluesql({
    engines: { scratch: { storage: 'memory' } },
    defaultEngine: 'scratch',
  });

  assert.deepEqual(db.listEngines(), ['memory', 'scratch']);
  assert.equal(db.defaultEngine(), 'scratch');
});

test('the default engine owns tables created without an ENGINE clause', async () => {
  const db = gluesql({
    engines: { scratch: { storage: 'memory' } },
    defaultEngine: 'scratch',
  });

  await db.query(`
    CREATE TABLE Implicit (id INTEGER);
    CREATE TABLE Explicit (id INTEGER) ENGINE = memory;
  `);

  db.setDefaultEngine('memory');
  db.removeEngine('scratch');

  // `Implicit` lived in the removed engine, `Explicit` did not.
  await assert.rejects(
    () => db.query('SELECT * FROM Implicit'),
    /table not found: Implicit/,
  );
  assert.deepEqual(await db.query('SELECT * FROM Explicit'), [
    { type: 'SELECT', rows: [] },
  ]);
});

test('routes tables by the ENGINE clause and joins across engines', async () => {
  const db = gluesql();
  db.addEngine('scratch', { storage: 'memory' });

  await db.query(`
    CREATE TABLE Main (mid INTEGER) ENGINE = memory;
    CREATE TABLE Scratch (sid INTEGER) ENGINE = scratch;
    INSERT INTO Main VALUES (1), (2);
    INSERT INTO Scratch VALUES (10);
  `);

  assert.deepEqual(await db.query('SELECT mid, sid FROM Main JOIN Scratch'), [
    {
      type: 'SELECT',
      rows: [
        { mid: 1, sid: 10 },
        { mid: 2, sid: 10 },
      ],
    },
  ]);
});

test('SHOW TABLES lists the tables of every engine', async () => {
  const db = gluesql();
  db.addEngine('scratch', { storage: 'memory' });

  await db.query(`
    CREATE TABLE Cached (id INTEGER) ENGINE = memory;
    CREATE TABLE Scratch (id INTEGER) ENGINE = scratch;
  `);

  assert.deepEqual(await db.query('SHOW TABLES'), [
    { type: 'SHOW TABLES', tables: ['Cached', 'Scratch'] },
  ]);

  db.removeEngine('scratch');

  assert.deepEqual(await db.query('SHOW TABLES'), [
    { type: 'SHOW TABLES', tables: ['Cached'] },
  ]);
});

test('removed engines drop out of the registry', async () => {
  const db = gluesql();
  db.addEngine('scratch', { storage: 'memory' });
  await db.query('CREATE TABLE Foo (id INTEGER) ENGINE = scratch');

  db.removeEngine('scratch');

  assert.deepEqual(db.listEngines(), ['memory']);
  await assert.rejects(
    () => db.query('SELECT * FROM Foo'),
    /table not found: Foo/,
  );
});

test('keeps the default engine registered', () => {
  const db = gluesql({ engines: { scratch: { storage: 'memory' } } });

  assert.throws(
    () => db.removeEngine('memory'),
    /cannot remove the default engine: memory \(call setDefaultEngine first\)/,
  );

  db.setDefaultEngine('scratch');
  db.removeEngine('memory');

  assert.deepEqual(db.listEngines(), ['scratch']);
  assert.equal(db.defaultEngine(), 'scratch');
});

test('rejects unknown engines and duplicated names', () => {
  const db = gluesql();

  assert.throws(
    () => db.addEngine('scratch', { storage: 'nope' }),
    /invalid storage config: unknown variant `nope`/,
  );
  assert.throws(
    () => db.addEngine('memory', { storage: 'memory' }),
    /engine already exists: memory/,
  );
  assert.throws(
    () => db.setDefaultEngine('scratch'),
    /engine not found: scratch \(registered: memory\)/,
  );
  assert.throws(() => db.removeEngine('scratch'), /engine not found: scratch/);
});

test('rejects engine names no ENGINE clause could reach', () => {
  const db = gluesql();

  for (const name of ['', '  ', 'my-db', '1st', 'a b']) {
    assert.throws(
      () => db.addEngine(name, { storage: 'memory' }),
      /invalid engine name/,
      `expected ${JSON.stringify(name)} to be rejected`,
    );
  }

  db.addEngine('my_db2', { storage: 'memory' });

  assert.deepEqual(db.listEngines(), ['memory', 'my_db2']);
});

test('rejects misspelled and misplaced config options', () => {
  const db = gluesql();

  // Would silently hand back a differently configured engine if unknown keys
  // were dropped.
  assert.throws(
    () => db.addEngine('disk', { storage: 'memory', path: './data' }),
    /invalid storage config: unknown field `path`/,
  );
});

test('supports custom functions', async () => {
  const db = gluesql();

  await db.query('CREATE FUNCTION add_one (n INT) RETURN n + 1');

  assert.deepEqual(await db.query('SELECT add_one(1) AS value'), [
    { type: 'SELECT', rows: [{ value: 2 }] },
  ]);
  assert.deepEqual(await db.query('SHOW FUNCTIONS'), [
    { type: 'SHOW FUNCTIONS', functions: ['add_one(n: INT)'] },
  ]);

  await db.query('DROP FUNCTION add_one');

  await assert.rejects(
    () => db.query('SELECT add_one(1) AS value'),
    /unsupported function: ADD_ONE/,
  );
});

test('reports table metadata of every engine', async () => {
  const db = gluesql();
  db.addEngine('scratch', { storage: 'memory' });

  await db.query(`
    CREATE TABLE Cached (id INTEGER) ENGINE = memory;
    CREATE TABLE Scratch (id INTEGER) ENGINE = scratch;
  `);

  const [{ rows }] = await db.query(
    'SELECT OBJECT_NAME, CREATED FROM GLUE_OBJECTS ORDER BY OBJECT_NAME',
  );

  assert.deepEqual(
    rows.map(({ OBJECT_NAME }) => OBJECT_NAME),
    ['Cached', 'Scratch'],
  );
  assert.match(rows[0].CREATED, /^\d{4}-\d{2}-\d{2} /);
});

test('reports the backends this build carries', () => {
  const compiled = storages();

  assert.deepEqual(compiled, [...compiled].sort());
  assert.ok(compiled.includes('memory'), 'every build carries the memory backend');
});
