import { execFileSync } from 'node:child_process';
import { appendFileSync } from 'node:fs';

const { GITHUB_REPOSITORY: repository, RELEASE_SHA: releaseSha, GITHUB_TOKEN: token, GITHUB_OUTPUT: outputPath } = process.env;
if (!repository || !releaseSha || !token || !outputPath) throw new Error('release attestation environment is incomplete');

const relevant = (path) => path === 'Cargo.toml' || path === 'Cargo.lock' || path.startsWith('v2/') || path.startsWith('scripts/ci/') || path === '.github/workflows/ci.yml';
const api = async (path) => {
  const response = await fetch(`https://api.github.com${path}`, { headers: { Accept: 'application/vnd.github+json', Authorization: `Bearer ${token}`, 'X-GitHub-Api-Version': '2022-11-28' } });
  if (!response.ok) throw new Error(`GitHub API failed: ${response.status} ${await response.text()}`);
  return response.json();
};
const ancestor = (left, right) => {
  try { execFileSync('git', ['merge-base', '--is-ancestor', left, right], { stdio: 'ignore' }); return true; } catch { return false; }
};
const changed = (left, right) => execFileSync('git', ['diff', '--name-only', left, right], { encoding: 'utf8' }).split('\n').filter(Boolean);

let accepted;
for (let page = 1; page <= 10 && !accepted; page += 1) {
  const result = await api(`/repos/${repository}/actions/workflows/ci.yml/runs?event=push&status=success&per_page=100&page=${page}`);
  for (const run of result.workflow_runs) {
    if (!ancestor(run.head_sha, releaseSha) || changed(run.head_sha, releaseSha).some(relevant)) continue;
    const artifacts = await api(`/repos/${repository}/actions/runs/${run.id}/artifacts?per_page=100`);
    if (artifacts.artifacts.some((artifact) => artifact.name === 'cargo-fui-release-inputs' && !artifact.expired)) {
      accepted = run;
      break;
    }
  }
  if (result.workflow_runs.length < 100) break;
}
if (!accepted) throw new Error('No non-expired successful cargo-fui CI attestation covers this release commit.');
appendFileSync(outputPath, `ci_run_id=${accepted.id}\n`);
console.log(`cargo-fui CI attestation: ${accepted.html_url}`);
