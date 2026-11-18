# GlueSQL.js

[![npm](https://img.shields.io/npm/v/gluesql)](https://www.npmjs.com/package/gluesql)
[![GitHub](https://img.shields.io/github/stars/gluesql/gluesql-js)](https://github.com/gluesql/gluesql-js)
[![LICENSE](https://img.shields.io/crates/l/gluesql.svg)](https://github.com/gluesql/gluesql-js/blob/main/LICENSE)
[![Chat](https://img.shields.io/discord/780298017940176946)](https://discord.gg/C6TDEgzDzY)
[![Coverage Status](https://coveralls.io/repos/github/gluesql/gluesql/badge.svg?branch=main)](https://coveralls.io/github/gluesql/gluesql?branch=main)

GlueSQL.js turns the browser into a SQL database. Real SQL — tables, joins,
aggregations — running inside the page: no server, no driver, no signup.
One `<script>` tag is a working database:

```html
<script type="module">
  import { gluesql } from 'https://cdn.jsdelivr.net/npm/gluesql/gluesql.js';

  const db = await gluesql();
  await db.query('CREATE TABLE Todo (id INTEGER, task TEXT);');
</script>
```

- **Zero backend** — ship local-first apps, prototypes, and internal tools
  with no infrastructure at all.
- **Offline & private by design** — every query runs on the device, and the
  data never leaves it.
- **Persistent** — OPFS-backed storage survives reloads and full browser
  restarts.
- **Multi-tab safe** — all tabs share one consistent database; tab crashes
  are handled for you.
- **Node.js too** — the same SQL runs in your tests and tooling.

## Pick the storage that matches your data

One SQL interface, four places to keep data:

| Your data is… | Keep it in | Import |
| --- | --- | --- |
| Scratch state & caches | Memory | `gluesql` (default) |
| Small settings that should stick | Web Storage | `gluesql` + `ENGINE = localStorage` |
| Real data — must survive restarts | OPFS file | `gluesql/opfs` |
| Real data, opened in many tabs | OPFS file, shared | `gluesql/opfs/shared` |

## Installation

#### Yarn
```
yarn add gluesql
```

#### npm
```
npm install gluesql
```

#### JavaScript modules
```javascript
import { gluesql } from 'https://cdn.jsdelivr.net/npm/gluesql/gluesql.js';
```

## Add it to your app

There is no schema server, migration daemon, or driver setup — adding GlueSQL
is adding one import. Pick the recipe that matches your stack:

**With a bundler (webpack, Vite, Rollup, …)** — install the package and
import it; the WASM engine ships inside:

```javascript
import { gluesql } from 'gluesql';

const db = await gluesql();
```

Working configurations live in
[`examples/web/webpack`](examples/web/webpack) and
[`examples/web/rollup`](examples/web/rollup) (Rollup uses the
`gluesql/gluesql.rollup` build).

**No build step at all** — the `<script type="module">` snippet at the top of
this page is the entire integration; see
[`examples/web/module`](examples/web/module) for a complete page.

**Node.js** — same package, same API:

```javascript
const { gluesql } = require('gluesql');
```

## Your first query

```javascript
import { gluesql } from 'gluesql';

const db = await gluesql();

await db.query(`
  CREATE TABLE User (id INTEGER, name TEXT);
  INSERT INTO User VALUES (1, "Hello"), (2, "World");
`);

const [{ rows }] = await db.query('SELECT * FROM User;');

console.log(rows);
```

## Mix storage engines in one query

In the main browser entry point, each table declares where it lives with the
`ENGINE` clause — and tables from different engines join like any others. Keep
a session cache in memory, user preferences in `localStorage`, and query them
together:

```javascript
import { gluesql } from 'gluesql';

const db = await gluesql();

await db.query(`
  CREATE TABLE Cache (id INTEGER, value TEXT) ENGINE = memory;
  CREATE TABLE Pref (id INTEGER, theme TEXT) ENGINE = localStorage;
  CREATE TABLE Draft (id INTEGER, body TEXT) ENGINE = sessionStorage;

  SELECT *
  FROM Cache
  JOIN Pref
  JOIN Draft;
`);
```

What each engine gives your users:

- `memory` — fastest, gone on reload. The default engine, also available in
  Node.js.
- `localStorage` — survives reloads and restarts, scoped to the origin.
- `sessionStorage` — survives reloads within one tab session, then cleans up
  after itself.

When the `ENGINE` clause is omitted, the current default engine (`memory`
initially) is used; change it with `db.setDefaultEngine('localStorage')`.

Web Storage is right for lightweight structured data. Browsers cap it at a
few MB per origin — for anything bigger, use the OPFS entry point below.

## Data that outlives the tab: `gluesql/opfs`

For an app's real data — the notes, records, and documents your users expect
to find again tomorrow — Web Storage is too small and memory is too
ephemeral. `gluesql/opfs` stores the database as a file in the
[Origin Private File System](https://developer.mozilla.org/en-US/docs/Web/API/File_System_API/Origin_private_file_system):
data survives page reloads and full browser restarts, and capacity follows
the browser's origin quota (typically gigabytes, not megabytes). GlueSQL runs
in a Dedicated Worker, so queries stay off your UI thread.

```javascript
import { gluesql } from 'gluesql/opfs';

const db = gluesql();

await db.query(`
  CREATE TABLE User (id INTEGER, name TEXT);
  INSERT INTO User VALUES (1, 'glue');
`);

// After a reload, the data is still there:
const [result] = await db.query('SELECT * FROM User');
```

Give each concern its own database with namespaces — separate files, separate
lifecycles:

```javascript
const app1 = gluesql({ namespace: 'app1' });
const app2 = gluesql({ namespace: 'app2' });
```

Notes:

- OPFS requires a [secure context](https://developer.mozilla.org/en-US/docs/Web/Security/Secure_Contexts) (HTTPS or localhost).
- This entry point provides OPFS as its only storage; the `ENGINE` clause
  and `setDefaultEngine` from the main browser entry point do not apply.
- A namespace can be opened by one context at a time — the underlying
  sync access handle is exclusive. If your users open multiple tabs, use
  `gluesql/opfs/shared` below.

Applying it to a project: the package ships the worker and WASM prebuilt, and
the entry point locates them relative to wherever the module itself is served
— no extra configuration when the package files are served from your own
origin (a dev server exposing `node_modules`, or the files copied into your
static assets). Browsers require workers to be same-origin, so the OPFS entry
points cannot be loaded straight from a third-party CDN; if your setup places
the worker elsewhere, point at it explicitly (the worker loads its WASM from
a `dist_opfs/` directory next to itself, so copy that along):

```javascript
const db = gluesql({ workerUrl: '/assets/gluesql.opfs.worker.js' });
```

A runnable demo with a persistent visit counter and an interactive SQL runner
lives in [`examples/web/opfs`](examples/web/opfs/README.md) — serve the
repository over `localhost` and open the page, no build tooling required.

### Every tab, one database: `gluesql/opfs/shared` (experimental)

Real users open your app in three tabs and expect them to agree. The OPFS
handle, however, is exclusive — only one context can own the file. Rather
than each tab failing to open the database (or you building cross-tab
coordination yourself), `gluesql/opfs/shared` does the coordination:

```javascript
import { gluesql } from 'gluesql/opfs/shared';

const db = gluesql({ namespace: 'app' });

await db.query(`CREATE TABLE IF NOT EXISTS Log (at TEXT);`);
```

Every tab calls `query()` as if it owned the database. Under the hood, tabs
elect a leader with a per-namespace
[Web Lock](https://developer.mozilla.org/en-US/docs/Web/API/Web_Locks_API);
only the leader spawns the database worker and holds the OPFS handle, and the
other tabs' queries reach it over a `BroadcastChannel`. A write committed in
one tab is immediately visible to queries from every other tab — one
database, not three copies drifting apart.

**When a tab dies, your app keeps working.** If the leader tab closes,
crashes, or is frozen by the browser, the browser releases its lock and the
next tab takes over automatically — queries issued in the meantime wait and
are delivered to the new leader. Failover is not free of edge cases, though,
and your app should know about one:

- A query that was already handed to the lost leader is rejected with a
  `leader lost` error, because no one can know whether it was applied —
  replaying it blindly could double-apply a write. Retry idempotent reads
  freely; guard non-idempotent writes at the application level. (In the
  instant of a hard crash an acceptance message can theoretically be lost,
  letting an automatic resend replay a query the leader had just started —
  the same guard applies.)

Remaining caveats, so you are not surprised in production:

- **Failover latency** — the lock is released as soon as the leader dies, but
  the new leader may need retry/backoff while the old OPFS handle is
  released.
- **Back/forward cache** — leaving a page terminates its connection so the
  handle can move on; a page restored from bfcache must create a new
  `gluesql()` instance.
- **No Web Locks or BroadcastChannel, no multi-tab** — where either API is
  unavailable, this entry point silently falls back to single-context
  `gluesql/opfs` behavior, including its one-tab limit.
- **Leader tab does the work** — queries from every tab execute in whichever
  tab currently leads; heavy queries consume that tab's CPU. Results are
  broadcast, so every tab on the namespace pays a copy of each result.
- **One protocol per origin** — the lock and channel are keyed only by
  namespace, so mixed app versions on one origin share them; keep the
  message protocol stable.

## The same SQL in Node.js

The `gluesql` package exposes the same API in Node.js, so schema and queries
written for the browser run unchanged in tests and scripts. In Node.js you also
register the engines yourself: each name you register is a storage backend, and
it is what the `ENGINE` clause of `CREATE TABLE` refers to.

| Your data is… | Keep it in | `storage` |
| --- | --- | --- |
| Scratch state & caches | Memory | `memory` (default) |

```javascript
const { gluesql } = require('gluesql');

// `memory` is always registered and starts out as the default engine.
const db = gluesql();

// Register engines up front...
const scratch = gluesql({
  engines: { scratch: { storage: 'memory' } },
  defaultEngine: 'scratch',
});

// ...or at any time later.
db.addEngine('scratch', { storage: 'memory' });

db.listEngines(); // ['memory', 'scratch']
db.defaultEngine(); // 'memory'
db.removeEngine('scratch');
```

Tables of different engines are queried together, exactly like the browser
build mixes `memory` with Web Storage:

```javascript
await db.query(`
  CREATE TABLE Cache (id INTEGER) ENGINE = memory;
  CREATE TABLE Docs (id INTEGER) ENGINE = scratch;

  SELECT * FROM Cache JOIN Docs;
`);
```

`removeEngine` only unregisters an engine; the data it owns is left untouched,
so registering the same backend again brings its tables back. The default
engine cannot be removed - point `setDefaultEngine` at another engine first.

Unknown keys in an engine config are rejected, so a misspelled option fails
loudly instead of quietly falling back to a default.

## License

This project is licensed under the Apache License, Version 2.0 - see the [LICENSE](https://raw.githubusercontent.com/gluesql/gluesql-js/main/LICENSE) file for details.

---

Docs: **<https://gluesql.org/docs>** —
[Getting Started (JavaScript)](https://gluesql.org/docs/dev/getting-started/javascript-web) ·
[Getting Started (Node.js)](https://gluesql.org/docs/dev/getting-started/nodejs) ·
[SQL Syntax](https://gluesql.org/docs/dev/sql-syntax/intro)
