export function gluesql(workerUrl = new URL('./gluesql.opfs.worker.js', import.meta.url)) {
  const worker = new Worker(workerUrl, { type: 'module' });
  const pending = new Map();
  let nextId = 0;
  let terminalError = null;

  // Once the worker can no longer respond (load failure or termination),
  // reject everything in flight and remember the error so later calls
  // fail immediately instead of waiting forever.
  const fail = (error) => {
    if (terminalError !== null) {
      return;
    }

    terminalError = error;

    for (const { reject } of pending.values()) {
      reject(error);
    }

    pending.clear();
  };

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
    fail(new Error(event.message ?? 'worker error'));
  };

  return {
    query(sql) {
      if (terminalError !== null) {
        return Promise.reject(terminalError);
      }

      return new Promise((resolve, reject) => {
        const id = nextId;
        nextId += 1;

        pending.set(id, { resolve, reject });
        worker.postMessage({ id, sql });
      });
    },
    terminate() {
      worker.terminate();
      fail(new Error('worker terminated'));
    },
  };
}
