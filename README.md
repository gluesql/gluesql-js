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
