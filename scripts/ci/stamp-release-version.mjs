import { readFileSync, writeFileSync } from 'node:fs';

const option = (name) => {
  const index = process.argv.indexOf(name);
  return index < 0 ? '' : process.argv[index + 1] ?? '';
};

const releaseVersion = option('--release-version');
if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/.test(releaseVersion)) {
  throw new Error(`invalid release version: ${JSON.stringify(releaseVersion)}`);
}

for (const path of ['v2/native/packaging/Cargo.toml', 'v2/cargo-fui/Cargo.toml']) {
  let source = readFileSync(path, 'utf8');
  let inPackage = false;
  let stamped = false;
  source = source.split('\n').map((line) => {
    const section = line.match(/^\[([^\]]+)\]$/);
    if (section) inPackage = section[1] === 'package';
    if (inPackage && !stamped && /^version\s*=/.test(line)) {
      stamped = true;
      return `version = "${releaseVersion}"`;
    }
    return line;
  }).join('\n');
  if (!stamped) throw new Error(`could not stamp ${path}`);
  writeFileSync(path, source);
}

const cargoPath = 'v2/cargo-fui/Cargo.toml';
let cargo = readFileSync(cargoPath, 'utf8');
const dependency = /(effindom-native-packaging = \{ path = "\.\.\/native\/packaging", version = )"[^"]+"/;
if (!dependency.test(cargo)) throw new Error('could not locate effindom-native-packaging dependency');
cargo = cargo.replace(dependency, `$1"${releaseVersion}"`);
writeFileSync(cargoPath, cargo);
