import init, { Glue } from './dist_opfs/gluesql_js.js';

let glue = null;

const ready = init().then(() => {
  glue = new Glue();
});

self.onmessage = async ({ data }) => {
  const { id, sql } = data;

  try {
    await ready;

    const result = await glue.query(sql);

    self.postMessage({ id, result });
  } catch (error) {
    self.postMessage({ id, error: String(error) });
  }
};
