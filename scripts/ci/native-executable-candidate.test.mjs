import assert from 'node:assert/strict';
import test from 'node:test';

import { nativeExecutableCandidateScore } from './native-executable-candidate.mjs';

test('selects supported Linux bundle executables', () => {
  assert.equal(nativeExecutableCandidateScore('/tmp/release/bundle/app/app', 'linux'), 100);
  assert.equal(nativeExecutableCandidateScore('/tmp/release/bundle/bin/app', 'linux'), 100);
  assert.equal(
    nativeExecutableCandidateScore('/tmp/release/bundle/sample/bin/sample', 'linux'),
    100,
  );
  assert.equal(
    nativeExecutableCandidateScore('/tmp/release/bundle/sample/app/sample', 'linux'),
    100,
  );
  assert.equal(
    nativeExecutableCandidateScore('/tmp/release/bundle/app.AppDir/usr/bin/app', 'linux'),
    100,
  );
});

test('rejects Linux libraries, nested resources, and non-release files', () => {
  assert.equal(nativeExecutableCandidateScore('/tmp/release/bundle/app/libapp.so', 'linux'), -1);
  assert.equal(
    nativeExecutableCandidateScore('/tmp/release/bundle/app/assets/fonts/font.ttf', 'linux'),
    -1,
  );
  assert.equal(
    nativeExecutableCandidateScore('/tmp/release/bundle/sample/share/app/helper', 'linux'),
    -1,
  );
  assert.equal(nativeExecutableCandidateScore('/tmp/debug/bundle/app/app', 'linux'), -1);
});

test('preserves macOS and Windows executable selection', () => {
  assert.equal(
    nativeExecutableCandidateScore('/tmp/release/bundle/App.app/Contents/MacOS/App', 'darwin'),
    100,
  );
  assert.equal(nativeExecutableCandidateScore('C:/tmp/release/bundle/app/app.exe', 'win32'), 100);
});
