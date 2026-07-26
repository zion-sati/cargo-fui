#!/usr/bin/env node

import { spawnSync } from 'node:child_process';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

export function classifyCargoPublish(status, output, crateName, version) {
  if (status === 0) return 'published';
  return output.includes(`crate ${crateName}@${version} already exists on crates.io index`)
    ? 'already-published'
    : 'failed';
}

function option(name) {
  const index = process.argv.indexOf(name);
  return index < 0 ? '' : process.argv[index + 1] ?? '';
}

function main() {
  const manifestPath = option('--manifest-path');
  const crateName = option('--crate');
  const version = option('--version');
  if (!manifestPath || !crateName || !version) process.exit(2);
  const result = spawnSync('cargo', ['publish', '--manifest-path', manifestPath, '--allow-dirty'], { encoding: 'utf8' });
  const output = `${result.stdout ?? ''}\n${result.stderr ?? ''}`;
  process.stdout.write(result.stdout ?? '');
  process.stderr.write(result.stderr ?? '');
  const outcome = classifyCargoPublish(result.status, output, crateName, version);
  if (outcome === 'already-published') {
    console.log(`${crateName}@${version} is already published; release replay accepted.`);
    return;
  }
  if (outcome === 'failed') process.exit(result.status ?? 1);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === resolve(process.argv[1])) main();
