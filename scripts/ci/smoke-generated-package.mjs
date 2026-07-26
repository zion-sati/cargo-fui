import { constants } from 'node:fs';
import { access, mkdtemp, readFile, readdir, rm } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const projectRoot = path.resolve(process.argv[2] ?? 'native-smoke');
const distRoot = path.join(projectRoot, 'dist');
const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'cargo-fui-package-smoke-'));

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

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: options.cwd,
    encoding: 'utf8',
    env: process.env,
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  if (result.error || result.status !== 0) {
    throw new Error(
      `${command} failed (${String(result.status)}): ${result.error?.message ?? ''}\nstdout:\n${result.stdout}\nstderr:\n${result.stderr}`,
    );
  }
  return result;
}

async function oneFile(extension) {
  const matches = (await collectFiles(distRoot)).filter(
    (file) => path.extname(file).toLowerCase() === extension,
  );
  if (matches.length !== 1) {
    throw new Error(`Expected one ${extension} package in ${distRoot}, found: ${matches.join(', ')}`);
  }
  return matches[0];
}

async function oneExecutable(root, predicate = () => true) {
  const matches = [];
  for (const file of await collectFiles(root)) {
    if (predicate(file) && (await isExecutable(file))) {
      matches.push(file);
    }
  }
  if (matches.length !== 1) {
    throw new Error(`Expected one packaged executable in ${root}, found: ${matches.join(', ')}`);
  }
  return matches[0];
}

async function smokeMacOs(screenshot) {
  const dmg = await oneFile('.dmg');
  const mount = path.join(temporaryRoot, 'mounted-dmg');
  await import('node:fs/promises').then(({ mkdir }) => mkdir(mount));
  run('/usr/bin/hdiutil', ['attach', '-quiet', '-readonly', '-nobrowse', '-mountpoint', mount, dmg]);
  try {
    const executable = await oneExecutable(
      mount,
      (file) => file.split(path.sep).join('/').includes('.app/Contents/MacOS/'),
    );
    run(executable, ['--hidden', '--screenshot', screenshot], { cwd: os.tmpdir() });
  } finally {
    run('/usr/bin/hdiutil', ['detach', '-quiet', mount]);
  }
}

async function smokeWindows(screenshot) {
  const msix = await oneFile('.msix');
  const unpacked = path.join(temporaryRoot, 'unpacked-msix');
  run('makeappx.exe', ['unpack', '/p', msix, '/d', unpacked, '/o']);
  const executable = await oneExecutable(unpacked);
  run(executable, ['--hidden', '--screenshot', screenshot], { cwd: path.dirname(executable) });
}

async function smokeLinux(screenshot) {
  const appImage = await oneFile('.appimage');
  const bytes = await readFile(appImage);
  const squashfsOffset = bytes.lastIndexOf(Buffer.from('hsqs'));
  if (squashfsOffset < 0) {
    throw new Error(`${appImage} contains no SquashFS payload.`);
  }
  const extracted = path.join(temporaryRoot, 'squashfs-root');
  run('unsquashfs', ['-f', '-d', extracted, '-o', String(squashfsOffset), appImage]);
  const appRun = path.join(extracted, 'AppRun');
  run('xvfb-run', ['--auto-servernum', appRun, '--hidden', '--screenshot', screenshot], {
    cwd: os.tmpdir(),
  });
}

async function assertScreenshot(file) {
  const bytes = await readFile(file);
  const pngSignature = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
  if (bytes.length < 10_000 || !bytes.subarray(0, 8).equals(pngSignature)) {
    throw new Error(`Packaged application produced an invalid or empty screenshot (${bytes.length} bytes).`);
  }
  const width = bytes.readUInt32BE(16);
  const height = bytes.readUInt32BE(20);
  if (width < 640 || height < 480) {
    throw new Error(`Packaged application screenshot is unexpectedly small: ${width}x${height}.`);
  }
  return { width, height, bytes: bytes.length };
}

const screenshot = path.join(temporaryRoot, 'packaged-app.png');
try {
  if (process.platform === 'darwin') {
    await smokeMacOs(screenshot);
  } else if (process.platform === 'win32') {
    await smokeWindows(screenshot);
  } else if (process.platform === 'linux') {
    await smokeLinux(screenshot);
  } else {
    throw new Error(`Unsupported package smoke platform: ${process.platform}.`);
  }
  const result = await assertScreenshot(screenshot);
  console.log(`Packaged application rendered ${result.width}x${result.height} (${result.bytes} bytes).`);
} finally {
  await rm(temporaryRoot, {
    recursive: true,
    force: true,
    maxRetries: process.platform === 'win32' ? 10 : 0,
    retryDelay: 200,
  });
}
