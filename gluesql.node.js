const native = require('./gluesql.native.js');

function gluesql() {
  const db = native.gluesql();

  return {
    async query(sql) {
      return JSON.parse(db.query(sql));
    },
  };
}

module.exports = { gluesql };
