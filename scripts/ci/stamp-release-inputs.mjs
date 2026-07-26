import { readFileSync, writeFileSync } from 'node:fs';

function option(name) {
  const index = process.argv.indexOf(name);
  return index < 0 ? '' : process.argv[index + 1] ?? '';
}

const inputsPath = option('--inputs');
const inputs = inputsPath
  ? JSON.parse(readFileSync(inputsPath, 'utf8'))
  : { fuiRsVersion: process.env.FUI_RS_VERSION, runtimeVersion: process.env.RUNTIME_VERSION };
const releaseVersion = option('--release-version');
for (const [name, value] of Object.entries(inputs)) {
  if (name === 'sha') continue;
  if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(value ?? '')) throw new Error(`invalid ${name}: ${JSON.stringify(value)}`);
}

const scaffoldPath = 'v2/cargo-fui/src/scaffold.rs';
let scaffold = readFileSync(scaffoldPath, 'utf8');
scaffold = scaffold.replace(/const FUI_RS_VERSION: &str = "[^"]+";/, `const FUI_RS_VERSION: &str = "${inputs.fuiRsVersion}";`);
scaffold = scaffold.replace(/const RUNTIME_VERSION: &str = "[^"]+";/, `const RUNTIME_VERSION: &str = "${inputs.runtimeVersion}";`);
writeFileSync(scaffoldPath, scaffold);

if (releaseVersion) {
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
}
