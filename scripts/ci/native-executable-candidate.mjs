import path from 'node:path';

const normalize = (file) => file.split(path.sep).join('/');

export function nativeExecutableCandidateScore(file, platform) {
  const normalized = normalize(file);
  if (!normalized.includes('/release/bundle/')) return -1;
  if (platform === 'darwin') {
    return normalized.includes('.app/Contents/MacOS/') ? 100 : -1;
  }
  if (platform === 'win32') return normalized.endsWith('.exe') ? 100 : -1;
  if (/\.(?:so|a|dylib)$/u.test(normalized)) return -1;
  if (normalized.includes('.AppDir/usr/bin/')) return 100;

  const bundleAppMarker = '/bundle/app/';
  if (normalized.includes(bundleAppMarker)) {
    const relativePath = normalized.split(bundleAppMarker, 2)[1];
    return relativePath && !relativePath.includes('/') ? 100 : -1;
  }
  return normalized.includes('/bundle/bin/') ? 100 : -1;
}
