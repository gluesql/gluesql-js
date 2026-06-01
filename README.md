# GlueSQL.js

[![npm](https://img.shields.io/npm/v/gluesql)](https://www.npmjs.com/package/gluesql)
[![GitHub](https://img.shields.io/github/stars/gluesql/gluesql-js)](https://github.com/gluesql/gluesql-js)
[![LICENSE](https://img.shields.io/crates/l/gluesql.svg)](https://github.com/gluesql/gluesql-js/blob/main/LICENSE)
[![Chat](https://img.shields.io/discord/780298017940176946)](https://discord.gg/C6TDEgzDzY)
[![Coverage Status](https://coveralls.io/repos/github/gluesql/gluesql/badge.svg?branch=main)](https://coveralls.io/github/gluesql/gluesql?branch=main)

GlueSQL.js is a SQL database for web browsers and Node.js. It works as an embedded database and entirely runs in the browser.
GlueSQL.js supports in-memory, localStorage, sessionStorage, and IndexedDB storage backends in browsers.


Learn more at the **<https://gluesql.org/docs>**

* [Getting Started - JavaScript](https://gluesql.org/docs/dev/getting-started/javascript-web)
* [Getting Started - Node.js](https://gluesql.org/docs/dev/getting-started/nodejs)
* [SQL Syntax](https://gluesql.org/docs/dev/sql-syntax/intro)

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

## Usage

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

## Browser Storage Engines

GlueSQL.js includes four browser storage engines:

| Engine | Persistence | Notes |
| --- | --- | --- |
| `memory` | In-memory only | Default engine. Data is cleared when the page is reloaded. |
| `localStorage` | Persistent per origin | Uses the browser `localStorage` API. |
| `sessionStorage` | Persistent for the browser tab session | Uses the browser `sessionStorage` API. |
| `indexedDB` | Persistent per origin | Requires `loadIndexedDB()` before use. |

Specify the storage engine with the `ENGINE` clause when creating a table:

```javascript
import { gluesql } from 'gluesql';

const db = await gluesql();
await db.loadIndexedDB();

await db.query(`
  CREATE TABLE Mem (id INTEGER) ENGINE = memory;
  CREATE TABLE Loc (id INTEGER) ENGINE = localStorage;
  CREATE TABLE Ses (id INTEGER) ENGINE = sessionStorage;
  CREATE TABLE Idb (id INTEGER) ENGINE = indexedDB;
`);
```

The browser package can query tables backed by different engines through the same SQL interface:

```javascript
const db = await gluesql();
await db.loadIndexedDB();

await db.query(`
  CREATE TABLE Mem (id INTEGER) ENGINE = memory;
  CREATE TABLE Loc (id INTEGER) ENGINE = localStorage;
  CREATE TABLE Ses (id INTEGER) ENGINE = sessionStorage;
  CREATE TABLE Idb (id INTEGER) ENGINE = indexedDB;

  SELECT *
  FROM Mem
  JOIN Loc
  JOIN Ses
  JOIN Idb;
`);
```

### Memory

`memory` is the default storage engine. It is available in browsers and Node.js.

```javascript
const db = await gluesql();

await db.query(`
  CREATE TABLE User (id INTEGER, name TEXT) ENGINE = memory;
`);
```

When the `ENGINE` clause is omitted, GlueSQL.js uses the current default engine. The initial default engine is `memory`.

### Web Storage

`localStorage` and `sessionStorage` are available in browsers and use the browser Web Storage APIs.

```javascript
const db = await gluesql();

await db.query(`
  CREATE TABLE LocalCache (id INTEGER, value TEXT) ENGINE = localStorage;
  CREATE TABLE SessionCache (id INTEGER, value TEXT) ENGINE = sessionStorage;
`);
```

- `localStorage` keeps data for the browser origin until it is explicitly cleared.
- `sessionStorage` keeps data for the current browser tab session.

Web Storage is useful for lightweight structured data. Browsers apply storage quotas, commonly around several MB per origin, so avoid using `localStorage` or `sessionStorage` for large datasets.

### IndexedDB

`indexedDB` is not loaded automatically. Call `loadIndexedDB()` before creating or querying tables with `ENGINE = indexedDB`:

```javascript
const db = await gluesql();

await db.loadIndexedDB();

await db.query(`
  CREATE TABLE User (id INTEGER, name TEXT) ENGINE = indexedDB;
  INSERT INTO User VALUES (1, 'glue'), (2, 'sql');
`);
```

GlueSQL.js handles IndexedDB version changes internally when table schemas change. Stored rows are converted to JSON-compatible values, so you can inspect them with the browser developer tools' IndexedDB viewer.

You can pass a namespace to `loadIndexedDB()` to isolate databases:

```javascript
await db.loadIndexedDB('my-app');
```

This namespace is used as the IndexedDB database name. Use different namespaces when you need isolated databases for tests, examples, or multiple applications on the same origin.

### Changing The Default Engine

The default engine is `memory`. In browsers, you can change it after creating the database:

```javascript
const db = await gluesql();

db.setDefaultEngine('localStorage');

await db.query(`
  CREATE TABLE User (id INTEGER, name TEXT);
`);
```

For IndexedDB, load it first before setting it as the default engine:

```javascript
const db = await gluesql();

await db.loadIndexedDB();
db.setDefaultEngine('indexedDB');
```

Node.js currently supports only non-persistent memory storage.

## License

This project is licensed under the Apache License, Version 2.0 - see the [LICENSE](https://raw.githubusercontent.com/gluesql/gluesql-js/main/LICENSE) file for details.
