const fs = require('node:fs');
const net = require('node:net');
const os = require('node:os');
const path = require('node:path');
const { storages } = require('../../gluesql.node.js');

/// Creates a temporary directory that is removed when the test finishes.
function tempDir(t) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'gluesql-node-'));

  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));

  return dir;
}

/// Path of a not-yet-existing file inside a temporary directory.
function tempPath(t, name) {
  return path.join(tempDir(t), name);
}

function reachable({ host, port, timeout = 500 }) {
  return new Promise((resolve) => {
    const socket = net
      .connect({ host, port })
      .setTimeout(timeout)
      .on('connect', () => {
        socket.destroy();
        resolve(true);
      })
      .on('timeout', () => {
        socket.destroy();
        resolve(false);
      })
      .on('error', () => resolve(false));
  });
}

/// Skips a test when this build was not compiled with the backend. Optional
/// backends (parquet, redis, mongo) are cargo features, so `npm run test:node`
/// stays green on a default build and the `full` build is what covers them.
function requireStorage(t, name) {
  if (storages().includes(name)) {
    return true;
  }

  t.skip(`build without the ${name} storage`);

  return false;
}

/// Skips a test when the server is down, unless GLUESQL_TEST_REQUIRE_SERVICES
/// is set: CI sets it so that an unreachable service fails the job instead of
/// silently reporting success.
async function requireServer(t, { name, host, port }) {
  if (await reachable({ host, port })) {
    return true;
  }

  const message = `no ${name} server on ${host}:${port}`;

  if (process.env.GLUESQL_TEST_REQUIRE_SERVICES) {
    throw new Error(message);
  }

  t.skip(message);

  return false;
}

module.exports = { tempDir, tempPath, requireStorage, requireServer };
