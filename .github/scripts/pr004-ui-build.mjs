import { execFileSync } from 'node:child_process';
import { copyFile, mkdir, mkdtemp, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { installProbePackages } from './pr004-ui-packages.mjs';

const sources = {
  amicro: {
    repo: 'Subhan-code/Amicro--Micro-transitions-',
    version: 'fa5f8ffbe39537a8db191806066b8773c47c82e9',
    files: ['registry/ui/entrance/fade-in.tsx', 'package-lock.json', 'LICENSE'],
  },
  glasswork: {
    repo: 'kubai087/glasswork',
    version: '4f0b6744b898b3a9b98fbd913ab24fdb83c53162',
    files: ['lens.js', 'glass.css', 'LICENSE', 'NOTICE'],
  },
  shadcn: {
    repo: 'satnaing/shadcn-admin',
    version: 'a156d9b90519348c4d71f0f0e6fcea179c49a43f',
    files: ['src/components/layout/main.tsx', 'src/lib/utils.ts', 'pnpm-lock.yaml', 'LICENSE'],
  },
};

function checkout(name, work) {
  const source = sources[name];
  const root = path.join(work, name);
  const git = (...args) => execFileSync('git', ['-C', root, ...args], { encoding: 'utf8' });
  execFileSync('git', ['init', '--quiet', root]);
  git('remote', 'add', 'origin', 'https://github.com/' + source.repo + '.git');
  git('fetch', '--quiet', '--no-tags', '--depth=1', '--filter=blob:none', 'origin', source.version);
  if (git('rev-parse', 'FETCH_HEAD').trim() !== source.version) throw Error('Unexpected source revision for ' + name);
  git('checkout', '--quiet', 'FETCH_HEAD', '--', ...source.files);
  return root;
}

const asImport = (root, file) => JSON.stringify(path.join(root, file).split(path.sep).join('/'));

function bundle(binary, root, entry, out, modules) {
  const env = { ...process.env, NODE_PATH: modules };
  execFileSync(binary, [entry, '--bundle', '--platform=browser', '--format=iife', '--jsx=automatic',
    '--minify', '--legal-comments=eof', '--define:process.env.NODE_ENV="production"', '--log-level=warning',
    '--tsconfig=' + path.join(root, 'tsconfig.json'), '--outfile=' + out], { env, stdio: 'inherit' });
}

async function build(work, output) {
  const amicro = checkout('amicro', work);
  const glass = checkout('glasswork', work);
  const shadcn = checkout('shadcn', work);
  const { amicroModules, shadcnModules, esbuildBinary } = await installProbePackages({
    amicroRoot: amicro, shadcnRoot: shadcn, work,
  });
  await writeFile(path.join(work, 'tsconfig.json'), JSON.stringify({ compilerOptions: {
    baseUrl: shadcn, paths: { '@/*': ['src/*'] }, jsx: 'react-jsx' },
  }));

  const amicroEntry = path.join(work, 'amicro-entry.tsx');
  await writeFile(amicroEntry, `
    import { createRoot } from 'react-dom/client';
    import { flushSync } from 'react-dom';
    import { FadeIn } from ${asImport(amicro, 'registry/ui/entrance/fade-in.tsx')};
    globalThis.mountAmicro = () => {
      const root = createRoot(document.querySelector('#amicro'));
      flushSync(() => root.render(<FadeIn duration={0.55} delay={0.12}>Transition</FadeIn>));
      return () => root.unmount();
    };
  `);
  bundle(esbuildBinary, work, amicroEntry, path.join(output, 'amicro.js'), amicroModules);

  const shadcnEntry = path.join(work, 'shadcn-entry.tsx');
  await writeFile(shadcnEntry, `
    import { createRoot } from 'react-dom/client';
    import { flushSync } from 'react-dom';
    import { Main } from ${asImport(shadcn, 'src/components/layout/main.tsx')};
    const root = createRoot(document.querySelector('#shadcn'));
    globalThis.mountShadcn = (fixed, fluid) => flushSync(() =>
      root.render(<Main fixed={fixed} fluid={fluid}>Layout</Main>));
  `);
  bundle(esbuildBinary, work, shadcnEntry, path.join(output, 'shadcn.js'), shadcnModules);

  // 从真实组件的类名编译样式，确保尺寸断言经过上游的查询与工具类。
  const requireShadcn = createRequire(path.join(shadcnModules, '..', 'probe.cjs'));
  const { compile } = requireShadcn('tailwindcss');
  const css = (file) => readFile(requireShadcn.resolve('tailwindcss/' + file), 'utf8');
  const compiler = await compile(await css('theme.css') + '\n' + await css('preflight.css') + '\n@tailwind utilities;');
  const mainSource = await readFile(path.join(shadcn, 'src/components/layout/main.tsx'), 'utf8');
  const candidates = [...new Set(mainSource.split(/[\s'"<>{}()]+/))];
  await writeFile(path.join(output, 'layout.css'), compiler.build([...candidates, '@container/content']));

  const glassEntry = path.join(work, 'glass-entry.js');
  await writeFile(glassEntry, 'import {liquidGlass} from ' + asImport(glass, 'lens.js') + ';globalThis.mountGlass=liquidGlass;');
  bundle(esbuildBinary, work, glassEntry, path.join(output, 'glass.js'), amicroModules);
  await copyFile(path.join(glass, 'glass.css'), path.join(output, 'glass.css'));

  const notices = [];
  for (const [name, root] of [['Amicro', amicro], ['Glasswork', glass], ['shadcn-admin', shadcn]]) {
    notices.push(name + '\n' + await readFile(path.join(root, 'LICENSE'), 'utf8'));
  }
  notices.push(await readFile(path.join(glass, 'NOTICE'), 'utf8'));
  for (const [modules, packages] of [
    [amicroModules, ['react', 'react-dom', 'scheduler', 'framer-motion', 'motion-dom', 'motion-utils', 'tslib']],
    [shadcnModules, ['react', 'react-dom', 'scheduler', 'clsx', 'tailwind-merge', 'tailwindcss']],
  ]) {
    for (const name of packages) {
      const dir = path.join(modules, name);
      const files = await readdir(dir);
      const license = files.find(file => /^licen[cs]e(\.txt|\.md)?$/i.test(file));
      if (!license) throw Error('Missing package license: ' + name);
      const { version } = JSON.parse(await readFile(path.join(dir, 'package.json'), 'utf8'));
      notices.push(name + '@' + version + '\n' + await readFile(path.join(dir, license), 'utf8'));
    }
  }
  await writeFile(path.join(output, 'NOTICES.txt'), notices.join('\n\n'));
  const here = fileURLToPath(new URL('.', import.meta.url));
  await copyFile(path.join(here, 'pr004-ui-checks.js'), path.join(output, 'checks.js'));
  await writeFile(path.join(output, 'index.html'), `<!doctype html><html lang="en"><head><meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self'; connect-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'">
<title>WinStrata component feasibility test</title>
<link rel="icon" href="data:,"><link rel="stylesheet" href="./layout.css"><link rel="stylesheet" href="./glass.css">
<style>body{overflow-x:hidden;background:repeating-linear-gradient(22deg,#193057 0 10px,#eff 10px 20px,#829 20px 30px)}#glass{width:240px;height:76px;margin:16px}#shadcn{width:1440px;background:white}</style>
<script defer src="./amicro.js"></script><script defer src="./glass.js"></script>
<script defer src="./shadcn.js"></script><script defer src="./checks.js"></script></head>
<body><div id="amicro"></div><button id="glass" type="button" class="glass" data-kind="light">Material</button>
<div id="shadcn" class="@container/content"></div></body></html>`);
}

if (process.argv.length !== 3) throw Error('Usage: node pr004-ui-build.mjs <empty output directory>');
const output = path.resolve(process.argv[2]);
await mkdir(output, { recursive: true });
if ((await readdir(output)).length) throw Error('Probe output directory must be empty.');
const work = await mkdtemp(path.join(tmpdir(), 'winstrata-ui-'));
try {
  await build(work, output);
  console.log('Built offline component probes from approved upstream commits.');
} finally {
  await rm(work, { recursive: true, force: true, maxRetries: 3 });
}
