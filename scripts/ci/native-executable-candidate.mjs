import path from 'node:path';

const normalize = (file) => file.split(path.sep).join('/');

export function nativeExecutableCandidateScore(file, platform) {
  const normalized = normalize(file);
  const bundleMarker = '/release/bundle/';
  if (!normalized.includes(bundleMarker)) return -1;
  if (platform === 'darwin') {
    return normalized.includes('.app/Contents/MacOS/') ? 100 : -1;
  }
  if (platform === 'win32') return normalized.endsWith('.exe') ? 100 : -1;
  if (/\.(?:so|a|dylib)$/u.test(normalized)) return -1;
  if (normalized.includes('.AppDir/usr/bin/')) return 100;

  const relativePath = normalized.split(bundleMarker, 2)[1];
  return /^(?:[^/]+\/)?(?:app|bin)\/[^/]+$/u.test(relativePath) ? 100 : -1;
}
