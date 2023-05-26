# `gluesql-js` has been moved to the GlueSQL main repo
## Link
[https://github.com/gluesql/gluesql/tree/main/gluesql-js](https://github.com/gluesql/gluesql/tree/main/pkg/javascript)


---
# GlueSQL-js
[![npm version](https://badge.fury.io/js/gluesql.svg)](https://badge.fury.io/js/gluesql)
[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

> Use SQL in web browsers!
* [Demo - https://gluesql.org/playground](https://gluesql.org/playground/)

GlueSQL-js provides 3 storage options
* In-memory
* LocalStorage
* SessionStorage

## :package: Installation
```
npm install gluesql
```

## :cloud: Usage
```javascript
const gluesql = import('gluesql');

async function main() {
  const { Glue } = await gluesql;
  const db = new Glue("memory");
  /* other options:
    const db = new Glue("localstorage", "{db-name}");
    const db = new Glue("sessionstorage", "{db-name}");
  */
  
  const sql = `
    CREATE TABLE Test (id INTEGER, name TEXT);
    INSERT INTO Test VALUES (101, "Glue");
    INSERT INTO Test VALUES (102, "Rust");
    INSERT INTO Test VALUES (103, "Yeah");
  `;
  
  await db.execute(sql);

  const items = (await db.execute("SELECT * FROM Test WHERE id < 103;"))[0];
  /* items:
    [
      [101, "Glue"],
      [102, "Rust"],
    ] 
  */
}
```

## :sparkles: Examples
* [GlueSQL JavaScript Seed](https://github.com/gluesql/gluesql-js-seed)
* [GlueSQL Web Dashboard Demo](https://github.com/gluesql/gluesql-js-demo)

## :books: Features

### :green_book: Supported Queries
* `CREATE TABLE`
* `ALTER TABLE`
* `INSERT`
* `UPDATE`
* `SELECT`
* `DELETE`
* `DROP TABLE`

### :blue_book: Supported Data Types & Attributes
#### Types
* `INTEGER`
* `FLOAT`
* `BOOLEAN`
* `TEXT`

#### Attributes
* `NULL` | `NOT NULL`

> Example
```sql
CREATE TABLE User (
  id INTEGER,
  name TEXT NULL,
  valid BOOLEAN
);
```

### :orange_book: Supported SQL Syntax Keywords
#### Join (only with `ON` keyword)
* `INNER JOIN` | `JOIN`
* `LEFT JOIN` | `LEFT OUTER JOIN`

> Example
```sql
SELECT * FROM TableA
JOIN TableB ON TableB.a_id = TableA.id
WHERE TableA.id > 10;
```

#### NestedSelect
> Example
```sql
SELECT * FROM User
WHERE User.id IN (SELECT id IN Other);
```

#### Aggregation
* `COUNT`
* `MAX`
* `MIN`
* `SUM`
* `GROUP BY`, `HAVING`

> Example
```sql
SELECT
  COUNT(*),
  MAX(amount) + MIN(amount),
  SUM(amount)
FROM TableA
GROUP BY city
HAVING COUNT(*) > 10;
```
