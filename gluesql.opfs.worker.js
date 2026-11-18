import init, { load } from './dist_opfs/gluesql_js.js';

let glue = null;

const namespace =
  new URL(self.location.href).searchParams.get('namespace') ?? undefined;

const ready = init()
  .then(() => load(namespace))
  .then((instance) => {
    glue = instance;
  });

// Requests are `{ id, sql }` for queries, `{ id, op: 'dump', options }`, or
// `{ id, op: 'restore', sql }`.
self.onmessage = async ({ data }) => {
  const { id, op, sql, options } = data;

  try {
    await ready;

    let result;

    if (op === 'dump') {
      result = await glue.dump(options);
    } else if (op === 'restore') {
      result = await glue.restore(sql);
    } else {
      result = await glue.query(sql);
    }

    self.postMessage({ id, result });
  } catch (error) {
    self.postMessage({ id, error: String(error) });
  }
};
