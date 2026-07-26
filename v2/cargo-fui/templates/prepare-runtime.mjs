import { cpSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';

const manifest = JSON.parse(readFileSync('node_modules/@effindomv2/runtime/dist/effindom.v2.manifest.json', 'utf8'));
if (typeof manifest.runtime_set_hash !== 'string' || manifest.runtime_set_hash.length === 0) {
  throw new Error('Installed EffinDOM runtime has no runtime_set_hash.');
}
rmSync('public', { recursive: true, force: true });
mkdirSync('public', { recursive: true });
cpSync('node_modules/@effindomv2/runtime/dist', 'public/runtime', { recursive: true });
cpSync('node_modules/@effindomv2/runtime/dist/bridge.js', 'public/bridge.js');
const shell = readFileSync('index.html', 'utf8')
  .replace('{{LOADING_OVERLAY_STYLES}}', readFileSync('loading-overlay-styles.html', 'utf8'))
  .replace('{{LOADING_OVERLAY_BODY}}', readFileSync('loading-overlay-body.html', 'utf8'));
writeFileSync('public/index.html', shell);
writeFileSync('public/effindom-runtime-config.js', `window.__effindomRuntime=${JSON.stringify({
  manifestUrls: [`https://runtimes.effindom.dev/v2/manifests/${manifest.runtime_set_hash}.json`, './runtime/effindom.v2.manifest.json'],
  expectedRuntimeSetHash: manifest.runtime_set_hash,
  buildMode: 'release',
})};\n`);
