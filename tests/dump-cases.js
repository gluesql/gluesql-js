// Round-trip cases for dump() and restore(), shared by the Node.js test
// (memory storage) and the browser test (redb on OPFS). Every case uses its
// own table names, so the cases can also be loaded into one database.

const single = (table, type, values) => {
  const rows = values.map((value, i) => `(${i}, ${value})`).join(', ');

  return `
    CREATE TABLE ${table} (id INT, v ${type});
    INSERT INTO ${table} VALUES ${rows}, (999, NULL);
  `;
};

const manyRows = Array.from({ length: 250 }, (_, i) => `(${i}, 'r${i}')`);

module.exports = [
  ['BOOLEAN', single('Bool', 'BOOLEAN', ['TRUE', 'FALSE'])],
  ['INT8 min/max', single('I8', 'INT8', ["CAST('-128' AS INT8)", '127'])],
  [
    'INT16 min/max',
    single('I16', 'INT16', ["CAST('-32768' AS INT16)", '32767']),
  ],
  [
    'INT32 min/max',
    single('I32', 'INT32', ["CAST('-2147483648' AS INT32)", '2147483647']),
  ],
  [
    'INT max',
    single('I64Max', 'INT', ['9223372036854775807', '-9223372036854775807']),
  ],
  [
    'INT min',
    single('I64Min', 'INT', ["CAST('-9223372036854775808' AS INT)"]),
  ],
  [
    'INT128 min/max',
    single('I128', 'INT128', [
      "CAST('-170141183460469231731687303715884105728' AS INT128)",
      "CAST('170141183460469231731687303715884105727' AS INT128)",
    ]),
  ],
  [
    'UINT8/16/32 max',
    `
      CREATE TABLE UintSmall (a UINT8, b UINT16, c UINT32);
      INSERT INTO UintSmall VALUES (255, 65535, 4294967295);
    `,
  ],
  [
    'UINT64 max',
    single('U64', 'UINT64', ["CAST('18446744073709551615' AS UINT64)"]),
  ],
  [
    'UINT128 max',
    single('U128', 'UINT128', [
      "CAST('340282366920938463463374607431768211455' AS UINT128)",
    ]),
  ],
  [
    'FLOAT32',
    single('F32', 'FLOAT32', [
      '1.5',
      "CAST('-0.1' AS FLOAT32)",
      "CAST('3.4028235e38' AS FLOAT32)",
    ]),
  ],
  [
    'FLOAT',
    single('F64', 'FLOAT', [
      '0.1',
      '-2.5',
      "CAST('1e300' AS FLOAT)",
      "CAST('-1.7976931348623157e308' AS FLOAT)",
      "CAST('5e-324' AS FLOAT)",
      "CAST('1e-7' AS FLOAT)",
    ]),
  ],
  ['FLOAT NaN', single('F64Nan', 'FLOAT', ["CAST('NaN' AS FLOAT)"])],
  [
    'FLOAT inf',
    single('F64Inf', 'FLOAT', ["CAST('inf' AS FLOAT)", "CAST('-inf' AS FLOAT)"]),
  ],
  [
    'FLOAT32 NaN/inf',
    single('F32Special', 'FLOAT32', [
      "CAST('NaN' AS FLOAT32)",
      "CAST('inf' AS FLOAT32)",
      "CAST('-inf' AS FLOAT32)",
      "CAST('-0.0' AS FLOAT32)",
    ]),
  ],
  [
    'FLOAT negative zero / integral',
    single('F64Zero', 'FLOAT', [
      "CAST('-0.0' AS FLOAT)",
      '1.0',
      '100000000000000000000.0',
    ]),
  ],
  [
    'DECIMAL',
    single('Dec', 'DECIMAL', [
      '3.14',
      "CAST('-0.000000000000000001' AS DECIMAL)",
      "CAST('79228162514264337593543950335' AS DECIMAL)",
    ]),
  ],
  [
    'TEXT plain/quotes',
    single('TextQuote', 'TEXT', [
      "'plain'",
      "'it''s'",
      "''",
      "'semi; -- dash /* c */'",
      "'\"double\"'",
    ]),
  ],
  [
    'TEXT newline/tab/backslash',
    single('TextSpace', 'TEXT', [
      "'line1\nline2'",
      "'tab\there'",
      "'back\\slash'",
      "'percent % under_score'",
    ]),
  ],
  [
    'TEXT unicode',
    single('TextUnicode', 'TEXT', ["'한글 테스트'", "'emoji 🙂 👩‍💻'"]),
  ],
  ['BYTEA', single('Bytes', 'BYTEA', ["X'00ff10'", "X''"])],
  ['INET', single('Inet', 'INET', ["'127.0.0.1'", "'::1'"])],
  ['DATE', single('Dates', 'DATE', ["DATE '2026-11-18'", "DATE '0001-01-01'"])],
  [
    'TIMESTAMP',
    single('Stamps', 'TIMESTAMP', [
      "TIMESTAMP '2026-11-18 13:53:00'",
      "TIMESTAMP '2026-11-18 13:53:00.123456'",
    ]),
  ],
  ['TIME', single('Times', 'TIME', ["TIME '23:59:59'", "TIME '00:00:00.001'"])],
  [
    'INTERVAL year-month',
    single('IntervalMonth', 'INTERVAL', [
      "INTERVAL '1' YEAR",
      "INTERVAL '1-2' YEAR TO MONTH",
      "INTERVAL '-3' MONTH",
    ]),
  ],
  [
    'INTERVAL day-time',
    single('IntervalTime', 'INTERVAL', [
      "INTERVAL '3 04:05:06.789' DAY TO SECOND",
      "INTERVAL '-5' MINUTE",
      "INTERVAL '36' HOUR",
    ]),
  ],
  [
    'UUID',
    single('Uuids', 'UUID', [
      "'d9b4f7e2-0c1a-4b6e-8f3d-2a1b3c4d5e6f'",
      'GENERATE_UUID()',
    ]),
  ],
  [
    'MAP',
    single('Maps', 'MAP', [
      `'{"a": {"b": [1, "x", null, true, 1.5]}}'`,
      `'{"quote": "it''s"}'`,
      `'{}'`,
    ]),
  ],
  [
    'MAP with float/nested quotes',
    single('MapNested', 'MAP', [
      String.raw`'{"f": 0.1, "s": "a\"b", "n": {"deep": [[1], [2, [3]]]}}'`,
    ]),
  ],
  ['LIST', single('Lists', 'LIST', [`'[1, "two", {"three": 3}]'`, `'[]'`])],
  [
    'POINT',
    single('Points', 'POINT', [
      'POINT(1.5, -2.25)',
      "POINT(CAST('NaN' AS FLOAT), CAST('-inf' AS FLOAT))",
    ]),
  ],
  [
    'schemaless table',
    `
      CREATE TABLE Schemaless;
      INSERT INTO Schemaless VALUES
        ('{"a": 1, "b": "x"}'),
        ('{"c": [1, 2], "d": null}');
    `,
  ],
  [
    'PRIMARY KEY / UNIQUE / NOT NULL',
    `
      CREATE TABLE Constrained (
        id INT PRIMARY KEY,
        email TEXT UNIQUE,
        name TEXT NOT NULL,
        note TEXT NULL
      );
      INSERT INTO Constrained VALUES (2, 'b@x', 'b', NULL), (1, 'a@x', 'a', 'n');
    `,
  ],
  [
    'DEFAULT literal/expression',
    `
      CREATE TABLE Defaults (
        id INT,
        n INT DEFAULT 7,
        s TEXT DEFAULT 'it''s',
        e INT DEFAULT 1 + 2,
        f FLOAT DEFAULT -1.5,
        b BOOLEAN DEFAULT TRUE
      );
      INSERT INTO Defaults (id) VALUES (1);
    `,
  ],
  [
    'COMMENT with quote',
    `
      CREATE TABLE Commented (id INT COMMENT 'user''s id') COMMENT = 'it''s a table';
      INSERT INTO Commented VALUES (1);
    `,
  ],
  [
    'FOREIGN KEY (referencing table sorts first)',
    `
      CREATE TABLE ZParent (id INT PRIMARY KEY);
      CREATE TABLE AChild (
        id INT,
        pid INT,
        FOREIGN KEY (pid) REFERENCES ZParent (id)
      );
      INSERT INTO ZParent VALUES (1);
      INSERT INTO AChild VALUES (10, 1);
    `,
  ],
  [
    'quoted identifiers',
    `
      CREATE TABLE "my table" ("from" INT, "Select" TEXT, "with space" INT);
      INSERT INTO "my table" VALUES (1, 'x', 2);
    `,
  ],
  [
    'several tables + empty table',
    `
      CREATE TABLE SeveralA (id INT);
      CREATE TABLE SeveralB (id INT, t TEXT);
      CREATE TABLE SeveralEmpty (id INT);
      INSERT INTO SeveralA VALUES (1), (2);
      INSERT INTO SeveralB VALUES (1, 'x');
    `,
  ],
  [
    '250 rows (batch boundary)',
    `
      CREATE TABLE ManyRows (id INT, t TEXT);
      INSERT INTO ManyRows VALUES ${manyRows.join(', ')};
    `,
  ],
].map(([name, sql]) => ({ name, sql }));
