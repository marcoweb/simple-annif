const fs = require('node:fs');
const assert = require('node:assert/strict');
const vm = require('node:vm');
// Development-only compatibility check. No browser upload or npm install.
// Usage: node verify_evametrics_import.cjs upstream.tsx exported.csv expected.json
const source = fs.readFileSync(process.argv[2], 'utf8');
// Extract only the reviewed import logic; remove its single TypeScript generic.
const start = source.indexOf("const rows = text.split");
const end = source.indexOf('const datasets = context.datasets', start);
assert(start >= 0 && end > start);
const parser = source.slice(start, end).replace('new Map<string, DatasetItem>()', 'new Map()');
const text = fs.readFileSync(process.argv[3], 'utf8');
const imported = vm.runInNewContext(parser + '\nJSON.stringify(Array.from(result.values()));', { text }, {timeout: 1000});
const rows = JSON.parse(imported);
assert.deepEqual(rows, JSON.parse(fs.readFileSync(process.argv[4], 'utf8')));
console.log('PASS: exported CSV imported by upstream parser:', JSON.stringify(rows));
