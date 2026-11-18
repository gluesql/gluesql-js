const native = require('./gluesql.native.js');

function gluesql(options = {}) {
  const { engines = {}, defaultEngine } = options;
  const db = native.gluesql();

  for (const [name, config] of Object.entries(engines)) {
    db.addEngine(name, config);
  }

  if (defaultEngine !== undefined) {
    db.setDefaultEngine(defaultEngine);
  }

  return {
    async query(sql) {
      return JSON.parse(db.query(sql));
    },
    addEngine(name, config) {
      db.addEngine(name, config);
    },
    removeEngine(name) {
      db.removeEngine(name);
    },
    listEngines() {
      return db.listEngines();
    },
    defaultEngine() {
      return db.defaultEngine();
    },
    setDefaultEngine(name) {
      db.setDefaultEngine(name);
    },
  };
}

module.exports = { gluesql, storages: native.storages };
