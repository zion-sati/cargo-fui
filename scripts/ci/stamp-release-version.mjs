import { readFileSync, writeFileSync } from 'node:fs';

const option = (name) => {
  const index = process.argv.indexOf(name);
  return index < 0 ? '' : process.argv[index + 1] ?? '';
};

const releaseVersion = option('--release-version');
if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/.test(releaseVersion)) {
  throw new Error(`invalid release version: ${JSON.stringify(releaseVersion)}`);
}

for (const path of ['v2/cargo-fui/Cargo.toml']) {
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
