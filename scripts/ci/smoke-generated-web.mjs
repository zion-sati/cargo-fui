import http from 'node:http';
import { readFile } from 'node:fs/promises';
import { extname, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const publicRoot = resolve(process.argv[2] ?? '');
const modulesRoot = resolve(process.argv[3] ?? '');
const { chromium } = await import(pathToFileURL(resolve(modulesRoot, 'playwright/index.mjs')));
const mime = new Map([
  ['.html', 'text/html; charset=utf-8'],
  ['.js', 'text/javascript; charset=utf-8'],
  ['.json', 'application/json'],
  ['.wasm', 'application/wasm'],
  ['.ttf', 'font/ttf'],
]);
const server = http.createServer(async (request, response) => {
  const requestPath = decodeURIComponent((request.url ?? '/').split('?')[0]).replace(/^\//, '') || 'index.html';
  if (requestPath.split('/').includes('..')) {
    response.writeHead(400).end('bad request');
    return;
  }
  try {
    const path = resolve(publicRoot, requestPath);
    const body = await readFile(path);
    response.writeHead(200, { 'Content-Type': mime.get(extname(path)) ?? 'application/octet-stream' }).end(body);
  } catch {
    response.writeHead(404).end('not found');
  }
});
await readFile(resolve(publicRoot, 'favicon.ico'));
await new Promise((resolveListening) => server.listen(8080, '127.0.0.1', resolveListening));

const browser = await chromium.launch({ headless: true });
try {
  const page = await browser.newPage({ viewport: { width: 900, height: 640 } });
  const errors = [];
  page.on('pageerror', (error) => errors.push(error.stack ?? error.message));
  page.on('console', (message) => { if (message.type() === 'error') errors.push(message.text()); });
  await page.goto('http://127.0.0.1:8080/', { waitUntil: 'networkidle', timeout: 30_000 });
  const canvas = page.locator('canvas');
  await canvas.waitFor({ state: 'visible', timeout: 30_000 });
  const size = await canvas.evaluate((element) => ({ width: element.width, height: element.height }));
  if (size.width < 800 || size.height < 560) throw new Error(`generated canvas did not follow the 900x640 viewport: ${JSON.stringify(size)}`);
  if (errors.length) throw new Error(`generated app emitted browser errors:\n${errors.join('\n')}`);
  console.log(`Generated browser app mounted a ${size.width}x${size.height} canvas without errors.`);
} finally {
  await browser.close();
  await new Promise((resolveClosed, reject) => server.close((error) => error ? reject(error) : resolveClosed()));
}
