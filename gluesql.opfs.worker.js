import init, { load } from './dist_opfs/gluesql_js.js';

let glue = null;

const namespace =
  new URL(self.location.href).searchParams.get('namespace') ?? undefined;

const ready = init()
  .then(() => load(namespace))
  .then((instance) => {
    glue = instance;
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
