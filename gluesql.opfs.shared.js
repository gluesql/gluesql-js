// Multi-tab OPFS entry point: tabs sharing a namespace elect a leader with
// the Web Locks API — the lock holder spawns `gluesql.opfs.worker.js` and
// owns the exclusive sync access handle — and exchange queries over a
// BroadcastChannel. The browser releases the lock when the leader dies, so
// failover needs no heartbeat.

import { gluesql as singleContext } from './gluesql.opfs.js';

const LEAD_ATTEMPTS = 8;
const LEAD_RETRY_BASE_MS = 100;
const RELEAD_DELAY_MS = 1000;

const AMBIGUOUS =
  'opfs-shared: leader lost; the query may or may not have been applied';

const delay = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

// Probe with backoff: right after failover the old handle may not be
// released yet.
const acquireWorker = async (workerUrl) => {
  let lastError = 'unknown';

  for (let attempt = 0; attempt < LEAD_ATTEMPTS; attempt += 1) {
    if (attempt > 0) {
      await delay(LEAD_RETRY_BASE_MS * 2 ** (attempt - 1));
    }

    const worker = new Worker(workerUrl, { type: 'module' });

    const probe = await new Promise((resolve) => {
      worker.onmessage = ({ data }) => resolve(data.error);
      worker.onerror = (event) =>
        resolve(event.message ?? 'worker failed to load');
      worker.postMessage({ id: 'probe', sql: 'SELECT 1' });
    });

    if (probe === undefined) {
      worker.onmessage = null;
      worker.onerror = null;

      return worker;
    }

    worker.terminate();
    lastError = probe;
  }

  throw new Error(lastError);
};

export function gluesql({ namespace, workerUrl } = {}) {
  const dbWorkerUrl = new URL(
    workerUrl ?? './gluesql.opfs.worker.js',
    import.meta.url,
  );

  if (namespace !== undefined) {
    dbWorkerUrl.searchParams.set('namespace', namespace);
  }

  if (
    typeof BroadcastChannel === 'undefined' ||
    typeof navigator === 'undefined' ||
    navigator.locks === undefined
  ) {
    return singleContext({ namespace, workerUrl });
  }

  // Lock and channel are origin-scoped, same as OPFS — one per namespace.
  const scope = `gluesql-opfs:${namespace ?? ''}`;
  const channel = new BroadcastChannel(scope);
  const tabId = crypto.randomUUID();

  const pending = new Map(); // id -> { resolve, reject, sql, acceptedBy }
  let nextSeq = 0;
  let terminalError = null;
  let closed = false;

  // ---- leader state, set only while this instance holds the lock ----
  let dbWorker = null;
  let epoch = null;
  let releaseLock = null;
  const handled = new Set(); // clients may resend after `leader-ready`

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

  // BroadcastChannel never delivers to its own sender, so leader-bound and
  // client-bound messages each take a local shortcut within this tab.
  const submit = (message) => {
    if (closed) {
      return;
    }

    if (epoch !== null) {
      onLeaderMessage(message);
    } else {
      channel.postMessage(message);
    }
  };

  const publish = (message) => {
    if (closed) {
      return;
    }

    channel.postMessage(message);
    onClientMessage(message);
  };

  const onClientMessage = (data) => {
    switch (data.type) {
      case 'accepted': {
        const request = pending.get(data.id);

        if (request !== undefined) {
          request.acceptedBy = data.epoch;
        }
        break;
      }
      case 'answer': {
        const request = pending.get(data.id);

        if (request !== undefined) {
          pending.delete(data.id);

          if (data.error === undefined) {
            request.resolve(data.result);
          } else {
            request.reject(new Error(data.error));
          }
        }
        break;
      }
      case 'leader-ready': {
        for (const [id, request] of pending) {
          if (request.acceptedBy === null) {
            // never reached a leader — safe to resend, the leader dedupes
            submit({ type: 'query', id, sql: request.sql });
          } else if (request.acceptedBy !== data.epoch) {
            // Replaying could double-apply: the lost leader may have run it.
            pending.delete(id);
            request.reject(new Error(AMBIGUOUS));
          }
        }
        break;
      }
      default:
        break;
    }
  };

  const onLeaderMessage = (data) => {
    switch (data.type) {
      case 'query': {
        if (!handled.has(data.id)) {
          handled.add(data.id);
          publish({ type: 'accepted', id: data.id, epoch });
          dbWorker.postMessage({ id: data.id, sql: data.sql });
        }
        break;
      }
      case 'who-leads': {
        publish({ type: 'leader-ready', epoch });
        break;
      }
      default:
        break;
    }
  };

  channel.onmessage = ({ data }) => {
    switch (data.type) {
      case 'query':
      case 'who-leads': {
        if (epoch !== null) {
          onLeaderMessage(data);
        }
        break;
      }
      default:
        onClientMessage(data);
    }
  };

  const stopLeading = () => {
    epoch = null;
    handled.clear();

    if (dbWorker !== null) {
      dbWorker.terminate();
      dbWorker = null;
    }

    if (releaseLock !== null) {
      releaseLock();
      releaseLock = null;
    }
  };

  const lead = async () => {
    let worker;

    try {
      worker = await acquireWorker(dbWorkerUrl);
    } catch (error) {
      // We held the exclusive lock, so any previous leader is gone; settle
      // our own pending instead of letting it hang.
      for (const [id, request] of pending) {
        pending.delete(id);
        request.reject(
          new Error(
            request.acceptedBy === null
              ? `opfs-shared: failed to lead: ${error.message ?? error}`
              : AMBIGUOUS,
          ),
        );
      }

      return;
    }

    if (closed) {
      worker.terminate();

      return;
    }

    dbWorker = worker;
    epoch = crypto.randomUUID();
    dbWorker.onmessage = ({ data }) =>
      publish({
        type: 'answer',
        id: data.id,
        error: data.error,
        result: data.result,
      });
    dbWorker.onerror = () => stopLeading(); // abdicate; the loop re-elects

    publish({ type: 'leader-ready', epoch });

    await new Promise((release) => {
      releaseLock = release;
    });
  };

  const leadLoop = async () => {
    while (!closed) {
      try {
        await navigator.locks.request(scope, async () => {
          if (!closed) {
            await lead();
          }
        });
      } catch {
        return; // lock manager unavailable (e.g. document tearing down)
      }

      if (!closed) {
        // Requests are granted in order, so waiting tabs go first; the
        // delay only keeps a lone tab with a failing lead from spinning.
        await delay(RELEAD_DELAY_MS);
      }
    }
  };

  const shutdown = (reason) => {
    if (closed) {
      return;
    }

    closed = true;
    stopLeading();
    channel.close();
    fail(new Error(reason));
  };

  // fires on close, navigation, and bfcache entry — stop leading in all cases
  addEventListener('pagehide', () => shutdown('page hidden'));
  // Chrome can freeze hidden tabs; a frozen leader would strand the OPFS
  // handle, so abdicate first and catch up on missed announcements at resume.
  addEventListener('freeze', () => stopLeading());
  addEventListener('resume', () => submit({ type: 'who-leads' }));

  leadLoop();

  return {
    query(sql) {
      if (terminalError !== null) {
        return Promise.reject(terminalError);
      }

      return new Promise((resolve, reject) => {
        const id = `${tabId}:${nextSeq}`;
        nextSeq += 1;

        pending.set(id, { resolve, reject, sql, acceptedBy: null });
        submit({ type: 'query', id, sql });
      });
    },
    terminate() {
      shutdown('connection terminated');
    },
  };
}
