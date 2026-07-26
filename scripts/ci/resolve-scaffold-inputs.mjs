import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const response = await fetch('https://crates.io/api/v1/crates/fui-rs', {
  headers: { 'User-Agent': 'cargo-fui-ci (https://github.com/zion-sati/cargo-fui)' },
});
if (!response.ok) throw new Error(`crates.io lookup failed: ${response.status} ${await response.text()}`);
const fuiRsVersion = (await response.json()).crate.max_version;
if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(fuiRsVersion)) {
  throw new Error(`crates.io returned invalid fui-rs version ${JSON.stringify(fuiRsVersion)}`);
}

const scratch = mkdtempSync(join(tmpdir(), 'cargo-fui-inputs-'));
writeFileSync(join(scratch, 'Cargo.toml'), `[package]\nname = "resolve-fui-input"\nversion = "0.0.0"\nedition = "2021"\n\n[lib]\npath = "lib.rs"\n\n[dependencies]\nfui-rs = "=${fuiRsVersion}"\n`);
writeFileSync(join(scratch, 'lib.rs'), '');
const metadata = JSON.parse(execFileSync('cargo', ['metadata', '--format-version', '1', '--manifest-path', join(scratch, 'Cargo.toml')], { encoding: 'utf8' }));
const fui = metadata.packages.find((item) => item.name === 'fui-rs' && item.version === fuiRsVersion);
const runtimeVersion = fui?.metadata?.effindom?.['runtime-version'];
if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(runtimeVersion ?? '')) {
  throw new Error(`fui-rs@${fuiRsVersion} does not declare valid package.metadata.effindom.runtime-version`);
}
const runtimeManifestUrl = `https://github.com/zion-sati/EffinDOM/releases/download/v${runtimeVersion}/native-runtime-manifest.json`;
const runtimeResponse = await fetch(runtimeManifestUrl, {
  headers: { 'User-Agent': 'cargo-fui-ci (https://github.com/zion-sati/cargo-fui)' },
});
if (!runtimeResponse.ok) {
  throw new Error(`fui-rs@${fuiRsVersion} declares EffinDOM ${runtimeVersion}, but its native runtime manifest is unavailable: ${runtimeManifestUrl} (${runtimeResponse.status})`);
}

writeFileSync(process.env.GITHUB_OUTPUT, `fui_rs_version=${fuiRsVersion}\nruntime_version=${runtimeVersion}\n`, { flag: 'a' });
console.log(`Resolved fui-rs@${fuiRsVersion} with EffinDOM runtime ${runtimeVersion}.`);
