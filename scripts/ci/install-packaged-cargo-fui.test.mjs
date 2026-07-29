import assert from 'node:assert/strict';
import test from 'node:test';

import { selectCargoFuiCrate } from './install-packaged-cargo-fui.mjs';

test('selects exactly one packaged cargo-fui source archive', () => {
  assert.equal(
    selectCargoFuiCrate([
      '/tmp/effindom-native-packaging-0.2.4.crate',
      '/tmp/cargo-fui-0.2.4-alpha1.crate',
    ]),
    '/tmp/cargo-fui-0.2.4-alpha1.crate',
  );
  assert.throws(() => selectCargoFuiCrate([]), /Expected one cargo-fui/u);
  assert.throws(
    () => selectCargoFuiCrate(['/tmp/cargo-fui-0.2.3.crate', '/tmp/cargo-fui-0.2.4.crate']),
    /Expected one cargo-fui/u,
  );
});
