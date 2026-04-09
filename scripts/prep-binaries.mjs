/**
 * prep-binaries.mjs
 * Copies sable-service.exe and sable-overlay.exe from the Cargo release target
 * into src-tauri/binaries/ with the target-triple suffix Tauri requires for
 * externalBin bundling.
 *
 * Run automatically as part of `npm run build:release`.
 */
import { execSync } from 'child_process';
import { copyFileSync, mkdirSync, existsSync } from 'fs';
import { join } from 'path';

const triple = execSync('rustc -vV').toString().match(/host:\s+(\S+)/)?.[1];
if (!triple) {
  console.error('Could not determine Rust host triple from `rustc -vV`');
  process.exit(1);
}

const binaries = ['sable-service', 'sable-overlay'];
const srcDir = join('target', 'release');
const dstDir = join('src-tauri', 'binaries');

mkdirSync(dstDir, { recursive: true });

for (const bin of binaries) {
  const src = join(srcDir, `${bin}.exe`);
  const dst = join(dstDir, `${bin}-${triple}.exe`);

  if (!existsSync(src)) {
    console.error(`Missing binary: ${src}`);
    console.error('Run `cargo build --release --workspace` first.');
    process.exit(1);
  }

  copyFileSync(src, dst);
  console.log(`Copied ${src} → ${dst}`);
}

console.log('Binaries ready for Tauri bundling.');
