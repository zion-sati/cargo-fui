import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const packageDirectory = dirname(dirname(fileURLToPath(import.meta.url)));
const scaffoldPath = join(packageDirectory, 'src', 'scaffold.rs');
const versionPattern = /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/;
const response = process.env.FUI_RS_VERSION ? null : await fetch('https://crates.io/api/v1/crates/fui-rs', {
  headers: { 'User-Agent': 'cargo-fui-updater (https://github.com/zion-sati/cargo-fui)' },
});
if (response && !response.ok) throw new Error(`crates.io lookup failed: ${response.status} ${await response.text()}`);
const fuiRsVersion = process.env.FUI_RS_VERSION ?? (await response.json()).crate.max_version;
if (!versionPattern.test(fuiRsVersion)) {
  throw new Error(`crates.io returned an invalid FUI-RS version: ${JSON.stringify(fuiRsVersion)}`);
}

const packagingResponse = process.env.EFFINDOM_NATIVE_PACKAGING_VERSION ? null : await fetch('https://crates.io/api/v1/crates/effindom-native-packaging', {
  headers: { 'User-Agent': 'cargo-fui-updater (https://github.com/zion-sati/cargo-fui)' },
});
if (packagingResponse && !packagingResponse.ok) {
  throw new Error(`crates.io packaging lookup failed: ${packagingResponse.status} ${await packagingResponse.text()}`);
}
const packagingVersion = process.env.EFFINDOM_NATIVE_PACKAGING_VERSION
  ?? (await packagingResponse.json()).crate.max_version;
if (!versionPattern.test(packagingVersion)) {
  throw new Error(`crates.io returned an invalid native-packaging version: ${JSON.stringify(packagingVersion)}`);
}

const scratch = mkdtempSync(join(tmpdir(), 'cargo-fui-updater-'));
writeFileSync(join(scratch, 'Cargo.toml'), `[package]\nname = "resolve-fui-input"\nversion = "0.0.0"\nedition = "2021"\n\n[lib]\npath = "lib.rs"\n\n[dependencies]\nfui-rs = "=${fuiRsVersion}"\n`);
writeFileSync(join(scratch, 'lib.rs'), '');
const metadata = JSON.parse(execFileSync('cargo', [
  'metadata',
  '--format-version',
  '1',
  '--manifest-path',
  join(scratch, 'Cargo.toml'),
], { encoding: 'utf8' }));
rmSync(scratch, { recursive: true, force: true });
const fui = metadata.packages.find((item) => item.name === 'fui-rs' && item.version === fuiRsVersion);
const runtimeVersion = process.env.EFFINDOM_RUNTIME_VERSION ?? fui?.metadata?.effindom?.['runtime-version'];
if (!versionPattern.test(runtimeVersion ?? '')) {
  throw new Error(`fui-rs@${fuiRsVersion} does not declare valid package.metadata.effindom.runtime-version.`);
}

const runtimeManifestUrl = `https://github.com/zion-sati/EffinDOM/releases/download/v${runtimeVersion}/native-runtime-manifest.json`;
const runtimeResponse = await fetch(runtimeManifestUrl, {
  headers: { 'User-Agent': 'cargo-fui-updater (https://github.com/zion-sati/cargo-fui)' },
});
if (!runtimeResponse.ok) {
  throw new Error(`The EffinDOM ${runtimeVersion} native runtime manifest is unavailable: ${runtimeManifestUrl} (${runtimeResponse.status})`);
}
await runtimeResponse.body?.cancel();

let scaffold = readFileSync(scaffoldPath, 'utf8');
scaffold = scaffold.replace(/const FUI_RS_VERSION: &str = "[^"]+";/, `const FUI_RS_VERSION: &str = "=${fuiRsVersion}";`);
scaffold = scaffold.replace(/const RUNTIME_VERSION: &str = "[^"]+";/, `const RUNTIME_VERSION: &str = "${runtimeVersion}";`);
writeFileSync(scaffoldPath, scaffold);

const cargoPath = join(packageDirectory, 'Cargo.toml');
let cargo = readFileSync(cargoPath, 'utf8');
const packagingDependency = /effindom-native-packaging\s*=\s*(?:"=[^"]+"|\{\s*path\s*=\s*"\.\.\/native\/packaging",\s*version\s*=\s*"[^"]+"\s*\})/;
if (!packagingDependency.test(cargo)) throw new Error('Could not locate effindom-native-packaging dependency.');
cargo = cargo.replace(packagingDependency, `effindom-native-packaging = "=${packagingVersion}"`);
writeFileSync(cargoPath, cargo);
execFileSync('cargo', ['generate-lockfile', '--manifest-path', cargoPath], { stdio: 'inherit' });
console.log(`Pinned fui-rs@=${fuiRsVersion}, EffinDOM runtime ${runtimeVersion}, and effindom-native-packaging@=${packagingVersion}.`);
