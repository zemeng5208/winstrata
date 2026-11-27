import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { createRequire } from 'node:module';
import { cp, mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '../..');
const pin = JSON.parse(await readFile(path.join(here, 'upstream.json'), 'utf8'));

function archiveName(name, version) {
  const file = name.startsWith('@') ? name.slice(name.indexOf('/') + 1) : name;
  return `${file}-${version}.tgz`;
}

function registryUrl(name, version) {
  return `https://registry.npmjs.org/${name}/-/${archiveName(name, version)}`;
}

async function unpack(modules, name, entry, archive) {
  if (!/^(?:@esbuild\/)?[a-z0-9][a-z0-9.-]*$/.test(name)) throw new Error(`Unexpected package name ${name}`);
  if (!/^[0-9]+\.[0-9]+\.[0-9]+$/.test(entry.version)) throw new Error(`Unexpected version for ${name}`);
  if (!/^sha512-[A-Za-z0-9+/]{86}==$/.test(entry.integrity)) throw new Error(`Unexpected integrity for ${name}`);
  const url = registryUrl(name, entry.version);
  execFileSync('curl', [
    '--proto', '=https', '--fail', '--show-error', '--silent', '--retry', '2',
    '--max-time', '90', '--output', archive, url,
  ], { stdio: 'inherit' });
  const digest = createHash('sha512').update(await readFile(archive)).digest('base64');
  if (`sha512-${digest}` !== entry.integrity) throw new Error(`Integrity mismatch for ${name}`);
  const dir = path.join(modules, name);
  await mkdir(dir, { recursive: true });
  execFileSync('tar', ['-xzf', archive, '--strip-components=1', '-C', dir], { stdio: 'inherit' });
  const manifest = JSON.parse(await readFile(path.join(dir, 'package.json'), 'utf8'));
  if (manifest.name !== name || manifest.version !== entry.version) {
    throw new Error(`Package identity mismatch for ${name}`);
  }
}

function bundle(binary, entry, outfile, tsconfig) {
  execFileSync(binary, [
    entry,
    '--bundle',
    '--platform=browser',
    '--format=iife',
    '--jsx=automatic',
    '--legal-comments=eof',
    '--define:process.env.NODE_ENV="production"',
    '--log-level=warning',
    `--tsconfig=${tsconfig}`,
    `--outfile=${outfile}`,
  ], { stdio: 'inherit' });
}

async function licenseText(dir) {
  const files = await readdir(dir);
  const license = files.find((file) => /^licen[cs]e(\.txt|\.md)?$/i.test(file));
  if (!license) throw new Error(`Missing license in ${dir}`);
  return readFile(path.join(dir, license), 'utf8');
}

if (process.argv.length !== 3) throw new Error('Usage: node packages/ui/build.mjs <output directory>');
if (!['linux', 'win32'].includes(process.platform) || process.arch !== 'x64') {
  throw new Error('The UI bundle build supports Linux x64 and Windows x64');
}

const output = path.resolve(process.argv[2]);
await mkdir(output, { recursive: true });
const cache = path.join(root, 'target', 'plan016-npm');
const amicroModules = path.join(cache, 'amicro', 'node_modules');
const shadcnModules = path.join(cache, 'shadcn', 'node_modules');
const archive = path.join(cache, 'package.tgz');
await mkdir(cache, { recursive: true });

const nativeName = `@esbuild/${process.platform}-x64`;
for (const [modules, table, names] of [
  [amicroModules, pin.bundles.amicro, ['react', 'react-dom', 'scheduler', 'framer-motion', 'motion-dom', 'motion-utils', 'tslib', 'esbuild', nativeName]],
  [shadcnModules, pin.bundles.shadcn, ['react', 'react-dom', 'scheduler', 'clsx', 'tailwind-merge', 'tailwindcss']],
]) {
  for (const name of names) await unpack(modules, name, table[name], archive);
}
await rm(archive, { force: true });

const nativeFile = process.platform === 'win32' ? 'esbuild.exe' : 'bin/esbuild';
const esbuild = path.join(amicroModules, nativeName, nativeFile);
const shadcnBuild = path.join(cache, 'shadcn');
await cp(path.join(root, 'packages/ui/vendor/amicro/fade-in.tsx'), path.join(cache, 'amicro', 'fade-in.tsx'));
await cp(
  path.join(root, 'packages/ui/vendor/shadcn-admin/src'),
  path.join(shadcnBuild, 'src'),
  { recursive: true },
);
const tsconfig = path.join(cache, 'tsconfig.json');
await writeFile(tsconfig, JSON.stringify({
  compilerOptions: {
    jsx: 'react-jsx',
    baseUrl: shadcnBuild,
    paths: { '@/*': ['src/*'] },
  },
}));

const amicroEntry = path.join(cache, 'amicro', 'entry.tsx');
await writeFile(amicroEntry, `
import { createRoot } from 'react-dom/client';
import { flushSync } from 'react-dom';
import { FadeIn } from './fade-in.tsx';
globalThis.mountAmicro = (node, html, motion) => {
  const root = createRoot(node);
  const render = (next, enabled) => flushSync(() => root.render(
    <FadeIn duration={enabled ? 0.55 : 0} delay={0}>
      <div id="page" dangerouslySetInnerHTML={{ __html: next }} />
    </FadeIn>
  ));
  render(html, motion);
  return { render, unmount: () => root.unmount() };
};
`);
bundle(esbuild, amicroEntry, path.join(output, 'amicro.js'), tsconfig);

const shadcnEntry = path.join(cache, 'shadcn', 'entry.tsx');
await writeFile(shadcnEntry, `
import { createRoot } from 'react-dom/client';
import { flushSync } from 'react-dom';
import { Main } from './src/components/layout/main.tsx';
const holder = { root: null };
globalThis.mountShadcn = (node, fixed, fluid) => {
  holder.root ??= createRoot(node);
  flushSync(() => holder.root.render(
    <Main fixed={fixed} fluid={fluid}><div id="page-slot"></div></Main>
  ));
  return node.querySelector('#page-slot');
};
`);
bundle(esbuild, shadcnEntry, path.join(output, 'shadcn.js'), tsconfig);

const glassEntry = path.join(cache, 'glass-entry.js');
await writeFile(glassEntry, `import { liquidGlass } from ${JSON.stringify(path.join(root, 'packages/ui/vendor/glasswork/lens.js'))}; globalThis.mountGlass = liquidGlass;`);
bundle(esbuild, glassEntry, path.join(output, 'glass.js'), tsconfig);

const requireShadcn = createRequire(path.join(shadcnModules, 'tailwindcss', 'package.json'));
const { compile } = requireShadcn('tailwindcss');
const css = (file) => readFile(requireShadcn.resolve(`tailwindcss/${file}`), 'utf8');
const compiler = await compile(`${await css('theme.css')}\n${await css('preflight.css')}\n@tailwind utilities;`);
const mainSource = await readFile(path.join(shadcnBuild, 'src/components/layout/main.tsx'), 'utf8');
const candidates = [...new Set(mainSource.split(/[\s'"<>{}()]+/))];
const built = compiler.build([...candidates, '@container/content']);
if (typeof built !== 'string' || !built.includes('max-w-7xl')) {
  throw new Error('Tailwind did not emit the shadcn-admin content layout utilities');
}
await writeFile(path.join(output, 'layout.css'), built);

const notices = [];
for (const source of pin.sources) {
  const license = source.files.find((file) => file.upstream === 'LICENSE');
  notices.push(`${source.name} ${source.version} ${source.revision}\n${await readFile(path.join(root, license.path), 'utf8')}`);
}
notices.push(await readFile(path.join(root, 'packages/ui/vendor/glasswork/NOTICE'), 'utf8'));
for (const [modules, table] of [[amicroModules, pin.bundles.amicro], [shadcnModules, pin.bundles.shadcn]]) {
  for (const [name, entry] of Object.entries(table)) {
    if (name.startsWith('@esbuild/') || name === 'esbuild') continue;
    notices.push(`${name}@${entry.version}\n${await licenseText(path.join(modules, name))}`);
  }
}
notices.push(`esbuild@${pin.bundles.amicro.esbuild.version}\n${await licenseText(path.join(amicroModules, 'esbuild'))}`);
await writeFile(path.join(output, 'NOTICES.txt'), notices.join('\n\n'));
console.log(`Built UI bundles in ${output}`);
