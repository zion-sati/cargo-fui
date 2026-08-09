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
  let requestPath = decodeURIComponent((request.url ?? '/').split('?')[0]).replace(/^\//, '') || 'index.html';
  if (requestPath.endsWith('/')) requestPath += 'index.html';
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
  let routes = [{ routePath: '/', wasmPath: '/app.wasm' }];
  try {
    routes = JSON.parse(await readFile(resolve(publicRoot, '..', 'routes.json'), 'utf8')).routes;
  } catch (error) {
    if (error?.code !== 'ENOENT') throw error;
  }
  const applicationModules = new Set([...routes.map((route) => route.wasmPath.split('/').at(-1)), 'app.wasm']);
  for (const route of routes) {
    const page = await browser.newPage({ viewport: { width: 900, height: 640 } });
    const errors = [];
    const requestedModules = [];
    page.on('pageerror', (error) => errors.push(error.stack ?? error.message));
    page.on('console', (message) => { if (message.type() === 'error') errors.push(message.text()); });
    page.on('response', (response) => {
      const url = new URL(response.url());
      const name = url.pathname.split('/').at(-1);
      if (applicationModules.has(name)) requestedModules.push(name);
      if (url.origin === 'http://127.0.0.1:8080' && response.status() >= 400) {
        errors.push(`${response.status()} ${url.pathname}`);
      }
    });
    await page.goto(`http://127.0.0.1:8080${route.routePath}`, { waitUntil: 'networkidle', timeout: 30_000 });
    const canvas = page.locator('canvas');
    await canvas.waitFor({ state: 'visible', timeout: 30_000 });
    const size = await canvas.evaluate((element) => ({ width: element.width, height: element.height }));
    if (size.width < 800 || size.height < 560) throw new Error(`generated canvas did not follow the 900x640 viewport: ${JSON.stringify(size)}`);
    const expectedModule = route.wasmPath.split('/').at(-1);
    if (requestedModules.length !== 1 || requestedModules[0] !== expectedModule) {
      errors.push(`expected only ${expectedModule}, requested ${requestedModules.join(', ') || 'none'}`);
    }
    if (errors.length) throw new Error(`generated app route ${route.routePath} emitted browser errors:\n${errors.join('\n')}`);
    await page.close();
  }
  console.log(`Generated browser app mounted ${routes.length} route(s) without errors.`);
} finally {
  await browser.close();
  await new Promise((resolveClosed, reject) => server.close((error) => error ? reject(error) : resolveClosed()));
}
