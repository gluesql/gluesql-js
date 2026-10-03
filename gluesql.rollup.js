import init, { Glue } from './dist_web/gluesql_js.js';
import loadDB from './dist_web/gluesql_js_bg.wasm';

// Shared by every gluesql() call; see gluesql.js.
let loading = null;

// `loadDB()` returns a promise, or the module itself when the wasm plugin's
// `sync` option covers this file; `await` accepts both.
async function instantiate() {
  return init(await loadDB());
}

function load() {
  if (loading === null) {
    loading = instantiate().catch((error) => {
      loading = null;

      throw error;
    });
  }

  return loading;
}

export async function gluesql() {
  await load();

  return new Glue();
}
