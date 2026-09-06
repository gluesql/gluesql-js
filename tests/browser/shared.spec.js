import { test, expect } from '@playwright/test';

const connect = async (page, namespace) => {
  await page.goto('/tests/browser/fixtures/blank.html');
  await page.evaluate(async (ns) => {
    const { gluesql } = await import('/gluesql.opfs.shared.js');
    window.db = gluesql({ namespace: ns });
  }, namespace);
};

const query = (page, sql) => page.evaluate((q) => window.db.query(q), sql);

// `leader lost` right after failover is documented policy — retry it here.
const queryWithFailoverRetry = (page, sql) =>
  page.evaluate(
    ([q, retries]) => {
      const run = (left) =>
        window.db.query(q).catch((error) => {
          if (left > 0 && error.message.includes('leader lost')) {
            return new Promise((resolve) => setTimeout(resolve, 500)).then(
              () => run(left - 1),
            );
          }

          throw error;
        });

      return run(retries);
    },
    [sql, 3],
  );

test('two tabs query the same namespace through one leader', async ({
  page,
  context,
}) => {
  await connect(page, 'shared-two-tabs');
  await query(
    page,
    `CREATE TABLE Item (id INTEGER, name TEXT);
     INSERT INTO Item VALUES (1, 'from-tab-a');`,
  );

  const pageB = await context.newPage();
  await connect(pageB, 'shared-two-tabs');

  const [seenByB] = await query(
    pageB,
    "SELECT * FROM Item ORDER BY id;",
  );
  expect(seenByB.rows).toEqual([{ id: 1, name: 'from-tab-a' }]);

  await query(pageB, "INSERT INTO Item VALUES (2, 'from-tab-b');");
  const [seenByA] = await query(page, 'SELECT * FROM Item ORDER BY id;');
  expect(seenByA.rows).toEqual([
    { id: 1, name: 'from-tab-a' },
    { id: 2, name: 'from-tab-b' },
  ]);

  const failure = await pageB.evaluate(() =>
    window.db.query('SELECT * FROM Missing').catch((error) => error.message),
  );
  expect(failure).toContain('table not found: Missing');
  const [alive] = await query(pageB, 'SELECT 1 AS one;');
  expect(alive.rows).toEqual([{ one: 1 }]);
});

test('routes concurrent answers back to each of three tabs', async ({
  page,
  context,
}) => {
  await connect(page, 'shared-three-tabs');
  await query(page, 'CREATE TABLE Seq (n INTEGER);');

  const pageB = await context.newPage();
  await connect(pageB, 'shared-three-tabs');
  const pageC = await context.newPage();
  await connect(pageC, 'shared-three-tabs');

  const [b, c] = await Promise.all([
    query(pageB, 'SELECT 2 AS me;'),
    query(pageC, 'SELECT 3 AS me;'),
  ]);
  expect(b[0].rows).toEqual([{ me: 2 }]);
  expect(c[0].rows).toEqual([{ me: 3 }]);

  await query(pageB, 'INSERT INTO Seq VALUES (1);');
  await query(pageC, 'INSERT INTO Seq VALUES (2);');
  const [total] = await query(page, 'SELECT COUNT(*) AS count FROM Seq;');
  expect(total.rows).toEqual([{ count: 2 }]);
});

test('closing the leader tab fails over to another tab', async ({
  page,
  context,
}) => {
  await connect(page, 'shared-failover');
  await query(
    page,
    `CREATE TABLE Note (id INTEGER);
     INSERT INTO Note VALUES (1);`,
  );

  const pageB = await context.newPage();
  await connect(pageB, 'shared-failover');
  const [before] = await queryWithFailoverRetry(
    pageB,
    'SELECT COUNT(*) AS count FROM Note;',
  );
  expect(before.rows).toEqual([{ count: 1 }]);

  await page.close();

  // Wait for the new leader with an idempotent read; retrying a write
  // after `leader lost` could double-apply.
  const [after] = await queryWithFailoverRetry(
    pageB,
    'SELECT COUNT(*) AS count FROM Note;',
  );
  expect(after.rows).toEqual([{ count: 1 }]);

  await query(pageB, 'INSERT INTO Note VALUES (2);');
  const [count] = await query(pageB, 'SELECT COUNT(*) AS count FROM Note;');
  expect(count.rows).toEqual([{ count: 2 }]);
});

test('a query sent while no tab leads waits for the next leader', async ({
  page,
  context,
}) => {
  await connect(page, 'shared-releader');
  await query(
    page,
    `CREATE TABLE Pending (id INTEGER);
     INSERT INTO Pending VALUES (1);`,
  );

  const pageB = await context.newPage();
  await connect(pageB, 'shared-releader');

  // The leader is gone and pageB has not led yet; the query must be resent
  // automatically once pageB acquires the lock — no retry helper.
  await page.close();
  const [rows] = await query(pageB, 'SELECT COUNT(*) AS count FROM Pending;');
  expect(rows.rows).toEqual([{ count: 1 }]);
});

test('falls back to single-context mode without BroadcastChannel', async ({
  context,
}) => {
  const page = await context.newPage();
  await page.addInitScript(() => {
    delete window.BroadcastChannel;
  });

  await connect(page, 'shared-fallback');
  const [result] = await query(page, 'SELECT 1 AS one;');
  expect(result.rows).toEqual([{ one: 1 }]);
});
