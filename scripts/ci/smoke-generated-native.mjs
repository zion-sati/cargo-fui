import { constants } from 'node:fs';
import { access, readdir, stat } from 'node:fs/promises';
import path from 'node:path';
import { spawn } from 'node:child_process';

const projectRoot = path.resolve(process.argv[2] ?? 'native-smoke');
const stagedRoot = path.join(projectRoot, 'target', 'fui');
const startupWindowMs = 2_000;
const shutdownWindowMs = 5_000;

const normalize = (value) => value.split(path.sep).join('/');

async function collectFiles(directory, output = []) {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const entryPath = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      await collectFiles(entryPath, output);
    } else if (entry.isFile()) {
      output.push(entryPath);
    }
  }
  return output;
}

async function isExecutable(file) {
  if (process.platform === 'win32') {
    return path.extname(file).toLowerCase() === '.exe';
  }
  try {
    await access(file, constants.X_OK);
    return true;
  } catch {
    return false;
  }
}

function candidateScore(file) {
  const normalized = normalize(file);
  if (!normalized.includes('/release/bundle/')) {
    return -1;
  }
  if (process.platform === 'darwin') {
    return normalized.includes('.app/Contents/MacOS/') ? 100 : -1;
  }
  if (process.platform === 'win32') {
    return normalized.endsWith('.exe') ? 100 : -1;
  }
  if (/\.(?:so|a|dylib)$/u.test(normalized)) {
    return -1;
  }
  if (normalized.includes('.AppDir/usr/bin/')) {
    return 100;
  }
  if (normalized.includes('/bundle/bin/')) {
    return 90;
  }
  return 10;
}

async function resolveExecutable() {
  const candidates = [];
  for (const file of await collectFiles(stagedRoot)) {
    const score = candidateScore(file);
    if (score >= 0 && (await isExecutable(file))) {
      candidates.push({ file, score });
    }
  }
  candidates.sort((left, right) => right.score - left.score || left.file.localeCompare(right.file));
  if (candidates.length === 0) {
    throw new Error(`No staged release application executable was found below ${stagedRoot}.`);
  }
  const best = candidates.filter(({ score }) => score === candidates[0].score);
  if (best.length !== 1) {
    throw new Error(`Expected one staged release application executable, found: ${best.map(({ file }) => file).join(', ')}`);
  }
  return best[0].file;
}

const executable = await resolveExecutable();
const command = process.platform === 'linux' ? 'xvfb-run' : executable;
const args = process.platform === 'linux' ? ['--auto-servernum', executable] : [];
const child = spawn(command, args, {
  cwd: path.dirname(executable),
  env: process.env,
  stdio: ['ignore', 'pipe', 'pipe'],
});

let stdout = '';
let stderr = '';
child.stdout.on('data', (chunk) => {
  stdout += chunk;
});
child.stderr.on('data', (chunk) => {
  stderr += chunk;
});

const exit = new Promise((resolve) => {
  child.once('error', (error) => resolve({ error }));
  child.once('exit', (code, signal) => resolve({ code, signal }));
});
const startup = await Promise.race([
  exit.then((result) => ({ kind: 'exit', result })),
  new Promise((resolve) => setTimeout(() => resolve({ kind: 'running' }), startupWindowMs)),
]);

if (startup.kind === 'exit') {
  throw new Error(
    `Generated native app exited during startup: ${JSON.stringify(startup.result)}\nstdout:\n${stdout}\nstderr:\n${stderr}`,
  );
}

child.kill();
const shutdown = await Promise.race([
  exit.then((result) => ({ kind: 'exit', result })),
  new Promise((resolve) => setTimeout(() => resolve({ kind: 'timeout' }), shutdownWindowMs)),
]);
if (shutdown.kind === 'timeout') {
  child.kill('SIGKILL');
  await exit;
}

const executableStats = await stat(executable);
console.log(
  `Generated native app stayed alive for ${startupWindowMs} ms: ${executable} (${executableStats.size} bytes).`,
);
