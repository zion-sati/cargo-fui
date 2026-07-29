#!/usr/bin/env node

import { mkdtemp, readdir, rm } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

function option(name) {
  const index = process.argv.indexOf(name);
  return index < 0 ? '' : process.argv[index + 1] ?? '';
}

export function selectCargoFuiCrate(files) {
  const matches = files.filter(
    (file) => /^cargo-fui-[0-9][0-9A-Za-z.+-]*\.crate$/u.test(path.basename(file)),
  );
  if (matches.length !== 1) {
    throw new Error(`Expected one cargo-fui .crate archive, found: ${matches.join(', ')}`);
  }
  return matches[0];
}

function run(command, args, cwd) {
  const result = spawnSync(command, args, { cwd, encoding: 'utf8', stdio: 'inherit' });
  if (result.error || result.status !== 0) {
    throw new Error(`${command} failed with exit code ${String(result.status)}: ${result.error?.message ?? ''}`);
  }
}

async function main() {
  const packageDirectory = path.resolve(option('--package-dir'));
  const installRoot = path.resolve(option('--install-root'));
  const packageFiles = (await readdir(packageDirectory)).map((file) => path.join(packageDirectory, file));
  const archive = selectCargoFuiCrate(packageFiles);
  const extractionRoot = await mkdtemp(path.join(os.tmpdir(), 'cargo-fui-package-'));

  try {
    run('tar', ['-xzf', archive, '-C', extractionRoot], process.cwd());
    const extractedDirectories = await readdir(extractionRoot, { withFileTypes: true });
    const packageRoots = extractedDirectories
      .filter((entry) => entry.isDirectory() && entry.name.startsWith('cargo-fui-'))
      .map((entry) => path.join(extractionRoot, entry.name));
    if (packageRoots.length !== 1) {
      throw new Error(`Expected one extracted cargo-fui package, found: ${packageRoots.join(', ')}`);
    }
    run('cargo', ['install', '--locked', '--path', packageRoots[0], '--root', installRoot], extractionRoot);
  } finally {
    await rm(extractionRoot, { recursive: true, force: true });
  }
}

if (process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1])) {
  await main();
}
