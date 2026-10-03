import { test, expect } from '@playwright/test';

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    window.instantiations = 0;

    for (const name of ['instantiate', 'instantiateStreaming']) {
      const original = WebAssembly[name];

      // Count only completed instantiations: wasm-bindgen retries a failed
      // instantiateStreaming (e.g. a server sending the wrong MIME type) with
      // instantiate, which would otherwise count one load twice.
      WebAssembly[name] = async function (...args) {
        const result = await original.apply(this, args);
        window.instantiations += 1;

        return result;
      };
    }
  });
  await page.goto('/tests/browser/fixtures/blank.html');
});

// React StrictMode runs effects twice, so the first two gluesql() calls
// commonly overlap before the WebAssembly module has finished loading.
test('concurrent gluesql() calls share one WebAssembly instance', async ({ page }) => {
  const result = await page.evaluate(async () => {
    const { gluesql } = await import('/gluesql.js');

    const first = gluesql();
    const second = gluesql();

    const a = await first;
    await a.query(`
      CREATE TABLE Foo (id INTEGER);
      INSERT INTO Foo VALUES (1);
    `);

    // Each handle has its own in-memory storage, so `b` cannot see Foo.
    const b = await second;
    const [{ rows: secondRows }] = await b.query('SELECT 1 AS one');

    const [{ rows: firstRows }] = await a.query('SELECT * FROM Foo');

    return { instantiations: window.instantiations, firstRows, secondRows };
  });

  expect(result).toEqual({
    instantiations: 1,
    firstRows: [{ id: 1 }],
    secondRows: [{ one: 1 }],
  });
});

// The shared loading promise must not cache a failure: once a load fails,
// the next gluesql() call starts a fresh one.
test('gluesql() can retry after the WebAssembly module fails to load', async ({ page }) => {
  const result = await page.evaluate(async () => {
    const { gluesql } = await import('/gluesql.js');

    const firstAttempt = await gluesql({ module_or_path: '/missing.wasm' }).then(
      () => 'resolved',
      () => 'rejected',
    );

    const db = await gluesql();
    const [{ rows }] = await db.query('SELECT 1 AS one');

    return { firstAttempt, rows };
  });

  expect(result).toEqual({ firstAttempt: 'rejected', rows: [{ one: 1 }] });
});
