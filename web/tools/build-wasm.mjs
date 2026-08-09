import { mkdirSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const webRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const repositoryRoot = resolve(webRoot, '..');
const outputDirectory = resolve(webRoot, 'public', 'wasm');
const wasmArtifact = resolve(
  repositoryRoot,
  'target',
  'wasm32-unknown-unknown',
  'release',
  'open_rtf_converter.wasm',
);

mkdirSync(outputDirectory, { recursive: true });
run(
  'cargo',
  ['build', '--release', '--target', 'wasm32-unknown-unknown', '--no-default-features'],
  repositoryRoot,
);
run(
  'wasm-bindgen',
  [
    wasmArtifact,
    '--target',
    'web',
    '--out-dir',
    outputDirectory,
    '--out-name',
    'open_rtf_converter',
  ],
  repositoryRoot,
);

function run(command, args, cwd) {
  const result = spawnSync(command, args, {
    cwd,
    stdio: 'inherit',
    shell: process.platform === 'win32',
  });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}
