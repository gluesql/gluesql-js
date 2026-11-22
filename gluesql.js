import init, { Glue } from './dist_web/gluesql_js.js';

// Shared by every gluesql() call, so overlapping calls (such as React
// StrictMode running effects twice) wait for one WebAssembly instance
// instead of each loading their own.
let loading = null;

function load(module_or_path) {
  if (loading === null) {
    loading = init(module_or_path).catch((error) => {
      loading = null;

      throw error;
    });
  }

  return loading;
}

export async function gluesql(module_or_path) {
  await load(module_or_path);

  return new Glue();
}
