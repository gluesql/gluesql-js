import { test, expect } from '@playwright/test';

test.beforeEach(async ({ page }) => {
  await page.goto('/tests/browser/fixtures/blank.html');
  await page.evaluate(async () => {
    const { gluesql } = await import('/gluesql.opfs.js');
    window.gluesql = gluesql;
  });
});

test('runs queries through the proxy/worker boundary', async ({ page }) => {
  const result = await page.evaluate(async () => {
    const db = window.gluesql();

    return db.query(`
      CREATE TABLE Foo (id INTEGER, name TEXT);
      INSERT INTO Foo VALUES (1, 'hello'), (2, 'worker');
      SELECT * FROM Foo ORDER BY id;
    `);
  });

  expect(result).toEqual([
    { type: 'CREATE TABLE' },
    { type: 'INSERT', affected: 2 },
    {
      type: 'SELECT',
      rows: [
        { id: 1, name: 'hello' },
        { id: 2, name: 'worker' },
      ],
    },
  ]);
});

test('propagates worker query errors and stays usable', async ({ page }) => {
  const result = await page.evaluate(async () => {
    const db = window.gluesql();

    const queryError = await db
      .query('SELECT * FROM Missing')
      .then(() => 'resolved')
      .catch((error) => error.message);

    const afterError = await db.query('SELECT 1 AS one');

    return { queryError, afterError };
  });

  expect(result.queryError).toContain('table not found: Missing');
  expect(result.afterError).toEqual([
    { type: 'SELECT', rows: [{ one: 1 }] },
  ]);
});

test('matches concurrent responses to their requests', async ({ page }) => {
  const results = await page.evaluate(async () => {
    const db = window.gluesql();

    await db.query(`
      CREATE TABLE Foo (id INTEGER, name TEXT);
      INSERT INTO Foo VALUES
        (0, 'row-0'), (1, 'row-1'), (2, 'row-2'), (3, 'row-3'), (4, 'row-4'),
        (5, 'row-5'), (6, 'row-6'), (7, 'row-7'), (8, 'row-8'), (9, 'row-9');
    `);

    const ids = Array.from({ length: 10 }, (_, i) => i);

    return Promise.all(
      ids.map((id) =>
        db
          .query(`SELECT name FROM Foo WHERE id = ${id}`)
          .then(([payload]) => payload.rows[0].name),
      ),
    );
  });

  expect(results).toEqual(
    Array.from({ length: 10 }, (_, i) => `row-${i}`),
  );
});

test('rejects in-flight and subsequent queries after terminate', async ({ page }) => {
  const result = await page.evaluate(async () => {
    const db = window.gluesql();

    const inFlight = db
      .query('SELECT 1 AS one')
      .then(() => 'resolved')
      .catch((error) => error.message);

    db.terminate();

    const afterTerminate = await db
      .query('SELECT 1 AS one')
      .then(() => 'resolved')
      .catch((error) => error.message);

    return { inFlight: await inFlight, afterTerminate };
  });

  expect(result.inFlight).toBe('worker terminated');
  expect(result.afterTerminate).toBe('worker terminated');
});

test('rejects queries when the worker fails to load', async ({ page }) => {
  const result = await page.evaluate(async () => {
    const db = window.gluesql(new URL('/does-not-exist.worker.js', location.origin));

    const initialFailure = await db
      .query('SELECT 1 AS one')
      .then(() => 'resolved')
      .catch(() => 'rejected');

    const afterFailure = await db
      .query('SELECT 1 AS one')
      .then(() => 'resolved')
      .catch(() => 'rejected');

    return { initialFailure, afterFailure };
  });

  expect(result.initialFailure).toBe('rejected');
  expect(result.afterFailure).toBe('rejected');
});
