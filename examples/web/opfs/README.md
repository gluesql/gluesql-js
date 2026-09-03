## Guide: Running opfs example

This example demonstrates the `gluesql/opfs` entry point: GlueSQL runs inside
a Dedicated Worker and stores data in a database file under the
[Origin Private File System](https://developer.mozilla.org/en-US/docs/Web/API/File_System_API/Origin_private_file_system),
so data survives page reloads and browser restarts.

### How to run?

1. Build the OPFS package from the repository root:

```sh
npm run build:opfs
```

2. Serve the repository root with any static http server:

```sh
python3 -m http.server 8765
```

3. Open <http://localhost:8765/examples/web/opfs/index.html> in a
   Chromium-based browser, Firefox, or Safari.

> OPFS requires a [secure context](https://developer.mozilla.org/en-US/docs/Web/Security/Secure_Contexts),
> so serve the page over `localhost` (as above) or HTTPS — opening the file
> directly via `file://` does not work.

### What to try

- **Persistence** — the page header shows `This page has been loaded N time(s)`.
  Every load inserts a row into the `Visit` table, so the counter grows on each
  reload and even survives a full browser restart. The data lives in an
  `example.db` file under the OPFS root.
- **SQL runner** — type any SQL into the textarea and press *Run*. Results with
  rows render as a table; other payloads print their type. For example:

  ```sql
  CREATE TABLE Todo (id INTEGER, task TEXT);
  INSERT INTO Todo VALUES (1, 'first'), (2, 'second');
  SELECT * FROM Todo;
  ```

  Reload the page — the `Todo` table is still there.
- **Errors** — running something like `SELECT * FROM Missing;` shows the error
  message inline in red.

### Inspecting and resetting the stored data

In Chromium DevTools, open *Application → Storage → Origin Private File System*
to see the `example.db` file. To start fresh, use *Clear site data* in the same
panel (or `navigator.storage.getDirectory()` from the console to remove entries
manually).
