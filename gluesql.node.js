const native = require('./gluesql.native.js');

function gluesql(options = {}) {
  if (Object.hasOwn(options, 'storage')) {
    if (!native.openRedb) {
      throw new Error('This build does not support opening redb files');
    }
    const db = native.openRedb(options);

    return {
      async query(sql) {
        return JSON.parse(db.query(sql));
      },
      close() {
        db.close();
      },
    };
  }

  const { engines = {}, defaultEngine } = options;
  if (defaultEngine !== undefined && typeof defaultEngine !== 'string') {
    throw new TypeError('defaultEngine must be a string');
  }
  const db = native.gluesql(Object.entries(engines), defaultEngine);

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
