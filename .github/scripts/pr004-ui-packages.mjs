import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdir, mkdtemp, readFile, rm } from 'node:fs/promises';
import path from 'node:path';

// 两个上游使用不同 React 锁定版本。只解包探针需要的文件依赖，不执行安装脚本。
const amicroPackages = [
  'react', 'react-dom', 'scheduler', 'framer-motion', 'motion-dom', 'motion-utils', 'tslib', 'esbuild',
];
const shadcnPackages = ['react', 'react-dom', 'scheduler', 'clsx', 'tailwind-merge', 'tailwindcss'];

function shadcnEntries(lock) {
  const packages = lock.split(/^packages:\s*$/m)[1]?.split(/^snapshots:\s*$/m)[0];
  if (!packages) throw new Error('Missing pnpm package resolutions');
  const entries = {};
  for (const name of shadcnPackages) {
    // 固定修订中这些包均只有一个版本。布局或解析结果有变化时必须重新审查。
    const pattern = new RegExp(
      `^  ${name}@([0-9]+\\.[0-9]+\\.[0-9]+):\\r?\\n    resolution: \\{integrity: (sha512-[A-Za-z0-9+/=]+)\\}\\s*$`,
      'gm',
    );
    const matches = Array.from(packages.matchAll(pattern));
    if (matches.length !== 1) throw new Error(`Expected one locked shadcn-admin resolution for ${name}`);
    const [, version, integrity] = matches[0];
    entries[name] = { version, integrity };
  }
  return entries;
}

async function unpackLocked(modules, name, entry, archive) {
  if (!entry || !/^[0-9]+\.[0-9]+\.[0-9]+$/.test(entry.version) ||
      !/^sha512-[A-Za-z0-9+/]{86}==$/.test(entry.integrity)) {
    throw new Error(`Invalid locked version or SHA-512 for ${name}`);
  }
  const fileName = name.split('/').at(-1);
  const url = `https://registry.npmjs.org/${name}/-/${fileName}-${entry.version}.tgz`;
  if (entry.resolved && entry.resolved !== url) throw new Error(`Unexpected tarball origin for ${name}`);
  execFileSync('curl', [
    '--proto', '=https', '--fail', '--show-error', '--silent', '--retry', '2',
    '--max-time', '90', '--output', archive, url,
  ], { stdio: 'inherit' });
  const digest = createHash('sha512').update(await readFile(archive)).digest('base64');
  if (`sha512-${digest}` !== entry.integrity) throw new Error(`Package integrity check failed for ${name}`);
  const dir = path.join(modules, name);
  await mkdir(dir, { recursive: true });
  execFileSync('tar', ['-xzf', archive, '--strip-components=1', '-C', dir], { stdio: 'inherit' });
  const manifest = JSON.parse(await readFile(path.join(dir, 'package.json'), 'utf8'));
  if (manifest.name !== name || manifest.version !== entry.version) {
    throw new Error(`Package identity mismatch for ${name}`);
  }
}

export async function installProbePackages({ amicroRoot, shadcnRoot, work }) {
  if (!['linux', 'win32'].includes(process.platform) || process.arch !== 'x64') {
    throw new Error('PR 004 UI package probe supports Linux x64 and Windows x64');
  }
  const native = `@esbuild/${process.platform}-x64`;
  const amicro = JSON.parse(await readFile(path.join(amicroRoot, 'package-lock.json'), 'utf8')).packages;
  const shadcn = shadcnEntries(await readFile(path.join(shadcnRoot, 'pnpm-lock.yaml'), 'utf8'));
  const stage = await mkdtemp(path.resolve(work, 'ui-packages-'));
  const amicroModules = path.join(stage, 'amicro', 'node_modules');
  const shadcnModules = path.join(stage, 'shadcn', 'node_modules');
  const archive = path.join(stage, 'package.tgz');
  try {
    for (const name of [...amicroPackages, native]) {
      await unpackLocked(amicroModules, name, amicro[`node_modules/${name}`], archive);
    }
    for (const [name, entry] of Object.entries(shadcn)) {
      await unpackLocked(shadcnModules, name, entry, archive);
    }
    await rm(archive);
    const nativeFile = process.platform === 'win32' ? 'esbuild.exe' : 'bin/esbuild';
    return { amicroModules, shadcnModules, esbuildBinary: path.join(amicroModules, native, nativeFile) };
  } catch (error) {
    await rm(stage, { recursive: true, force: true });
    throw error;
  }
}
