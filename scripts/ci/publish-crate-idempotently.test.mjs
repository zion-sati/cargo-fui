import assert from 'node:assert/strict';
import test from 'node:test';
import { classifyCargoPublish } from './publish-crate-idempotently.mjs';

test('accepts successful and exact duplicate publication outcomes only', () => {
  assert.equal(classifyCargoPublish(0, '', 'cargo-fui', '1.2.3'), 'published');
  assert.equal(classifyCargoPublish(101, 'crate cargo-fui@1.2.3 already exists on crates.io index', 'cargo-fui', '1.2.3'), 'already-published');
  assert.equal(classifyCargoPublish(101, 'network failure', 'cargo-fui', '1.2.3'), 'failed');
});
