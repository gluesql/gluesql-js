[![npm version](https://badge.fury.io/js/gluesql.svg)](https://badge.fury.io/js/gluesql)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
# GlueSQL-js
Use SQL in web browsers!

GlueSQL-js provides 3 storage options
* In-memory
* LocalStorage
* SessionStorage

## Installation
```
npm install gluesql
```

## Basic Usage
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
  
  db.execute(sql);

  const items = db.execute("SELECT * FROM Test WHERE id < 103;")[0];
  /* items:
    [
      [101, "Glue"],
      [102, "Rust"],
    ] 
  */
}
```

## Other Examples
* [GlueSQL JavaScript Seed](https://github.com/gluesql/gluesql-js-seed)
