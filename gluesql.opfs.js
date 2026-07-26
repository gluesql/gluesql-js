export function gluesql(workerUrl = new URL('./gluesql.opfs.worker.js', import.meta.url)) {
  const worker = new Worker(workerUrl, { type: 'module' });
  const pending = new Map();
  let nextId = 0;

  worker.onmessage = ({ data }) => {
    const { id, result, error } = data;
    const request = pending.get(id);

    if (!request) {
      return;
    }

    pending.delete(id);

    if (error === undefined) {
      request.resolve(result);
    } else {
      request.reject(new Error(error));
    }
  };

  worker.onerror = (event) => {
    for (const { reject } of pending.values()) {
      reject(new Error(event.message ?? 'worker error'));
    }

    pending.clear();
  };

  return {
    query(sql) {
      return new Promise((resolve, reject) => {
        const id = nextId;
        nextId += 1;

        pending.set(id, { resolve, reject });
        worker.postMessage({ id, sql });
      });
    },
    terminate() {
      worker.terminate();
    },
  };
}
