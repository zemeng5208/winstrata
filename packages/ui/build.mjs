import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { createRequire } from 'node:module'
import { mkdir, mkdtemp, readFile, readdir, rm, writeFile, copyFile, stat } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

const here = path.dirname(fileURLToPath(import.meta.url))
const output = path.resolve(here, 'dist')
const shadcnRevision = 'a156d9b90519348c4d71f0f0e6fcea179c49a43f'
const amicroRevision = 'fa5f8ffbe39537a8db191806066b8773c47c82e9'
const roots = [
  'react', 'react-dom', 'clsx', 'tailwind-merge', 'tailwindcss',
  'class-variance-authority', 'lucide-react', '@radix-ui/react-slot',
  '@radix-ui/react-dialog', '@radix-ui/react-separator', '@radix-ui/react-tooltip',
  '@radix-ui/react-avatar', '@radix-ui/react-collapsible', '@radix-ui/react-dropdown-menu',
  '@tanstack/react-router', 'tw-animate-css',
]

function sha256(bytes) {
  return createHash('sha256').update(bytes).digest('hex')
}

function curl(url, destination) {
  execFileSync('curl', [
    '--proto', '=https', '--fail', '--silent', '--show-error', '--retry', '2',
    '--max-time', '120', '--output', destination, url,
  ], { stdio: 'inherit' })
}

function parsePnpm(text) {
  const integrity = new Map()
  const snapshots = new Map()
  const importers = new Map()
  let section = ''
  let key = ''
  let inDeps = false
  const lines = text.split('\n')
  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index]
    if (line === 'packages:') { section = 'packages'; key = ''; inDeps = false; continue }
    if (line === 'snapshots:') { section = 'snapshots'; key = ''; inDeps = false; continue }
    if (line === 'importers:') { section = 'importers'; key = ''; inDeps = false; continue }
    if (section === 'packages' && /^ {2}\S.*:$/.test(line)) {
      key = line.slice(2, -1).replaceAll("'", '')
      inDeps = false
      continue
    }
    if (section === 'packages' && key && line.includes('integrity: sha512-')) {
      const match = line.match(/integrity: (sha512-[A-Za-z0-9+/=]+)/)
      if (match) integrity.set(key, match[1])
      continue
    }
    if (section === 'snapshots' && /^ {2}\S.*:\s*\{\}\s*$/.test(line)) {
      key = line.slice(2, line.indexOf(':')).trim().replaceAll("'", '')
      snapshots.set(key, [])
      inDeps = false
      continue
    }
    if (section === 'snapshots' && /^ {2}\S.*:$/.test(line)) {
      key = line.slice(2, -1).replaceAll("'", '')
      snapshots.set(key, [])
      inDeps = false
      continue
    }
    if (section === 'snapshots' && key && line === '    dependencies:') { inDeps = true; continue }
    if (section === 'snapshots' && key && inDeps && /^ {6}\S/.test(line)) {
      const match = line.match(/^ {6}'?(.+?)'?: (.+)$/)
      if (match) snapshots.get(key).push([match[1], match[2]])
      continue
    }
    if (section === 'snapshots' && key && inDeps && !line.startsWith('      ')) inDeps = false
    if (section === 'importers' && /^ {6}\S.*:$/.test(line) && !line.startsWith('        ')) {
      key = line.slice(6, -1).replaceAll("'", '')
      continue
    }
    if (section === 'importers' && key && line.startsWith('        version: ')) {
      importers.set(key, line.slice('        version: '.length).trim())
    }
  }
  return { integrity, snapshots, importers }
}

function parseNpmLock(text) {
  const json = JSON.parse(text)
  const packages = json.packages ?? {}
  const entries = new Map()
  for (const [name, entry] of Object.entries(packages)) {
    if (!name.startsWith('node_modules/')) continue
    const packageName = name.slice('node_modules/'.length)
    if (packageName.startsWith('.bin') || packageName.includes('/node_modules/')) continue
    entries.set(packageName, entry)
  }
  return entries
}

function tarball(name, version) {
  const fileName = name.startsWith('@') ? name.slice(name.indexOf('/') + 1) : name
  return `https://registry.npmjs.org/${name}/-/${fileName}-${version}.tgz`
}

async function unpack(modules, name, version, expected, archive) {
  const url = tarball(name, version)
  curl(url, archive)
  const digest = `sha512-${createHash('sha512').update(await readFile(archive)).digest('base64')}`
  if (digest !== expected) throw new Error(`integrity mismatch for ${name}@${version}`)
  const destination = path.join(modules, ...name.split('/'))
  await mkdir(destination, { recursive: true })
  execFileSync('tar', ['-xzf', archive, '--strip-components=1', '-C', destination], { stdio: 'inherit' })
  const manifest = JSON.parse(await readFile(path.join(destination, 'package.json'), 'utf8'))
  if (manifest.name !== name || manifest.version !== version) {
    throw new Error(`package identity mismatch for ${name}`)
  }
}

function bareVersion(spec) {
  const open = spec.indexOf('(')
  return open === -1 ? spec : spec.slice(0, open)
}

async function unpackPnpmTree(modules, parsed, archive) {
  const seen = new Set()
  const visit = async (snapshotKey) => {
    if (seen.has(snapshotKey)) return
    if (!parsed.snapshots.has(snapshotKey)) throw new Error(`missing snapshot ${snapshotKey}`)
    seen.add(snapshotKey)
    const at = snapshotKey.search(/@\d/)
    if (at < 1) throw new Error(`cannot read package version in ${snapshotKey}`)
    const name = snapshotKey.slice(0, at)
    const version = bareVersion(snapshotKey.slice(at + 1))
    const packageKey = `${name}@${version}`
    const expected = parsed.integrity.get(packageKey)
    if (!expected) throw new Error(`missing integrity for ${packageKey}`)
    await unpack(modules, name, version, expected, archive)
    for (const [dependency, spec] of parsed.snapshots.get(snapshotKey)) {
      await visit(`${dependency}@${spec}`)
    }
  }
  for (const name of roots) {
    const spec = parsed.importers.get(name)
    if (!spec) throw new Error(`shadcn-admin lock has no ${name}`)
    await visit(`${name}@${spec}`)
  }
}

async function unpackNpmTree(modules, entries, name, archive, seen = new Set()) {
  if (seen.has(name)) return
  seen.add(name)
  const entry = entries.get(name)
  if (!entry?.integrity || !entry.version) throw new Error(`amicro lock has no ${name}`)
  await unpack(modules, name, entry.version, entry.integrity, archive)
  for (const dependency of Object.keys(entry.dependencies ?? {})) {
    await unpackNpmTree(modules, entries, dependency, archive, seen)
  }
}

async function compileCss(modules, candidates) {
  const requireFrom = createRequire(path.join(modules, 'tailwindcss', 'package.json'))
  const tailwind = requireFrom('tailwindcss')
  const compile = tailwind.compile
  if (typeof compile !== 'function') throw new Error('tailwindcss compile export is missing')
  const readPackageCss = async (id) => {
    if (id === 'tailwindcss') return readFile(requireFrom.resolve('tailwindcss/index.css'), 'utf8')
    if (id === 'tw-animate-css') {
      const dir = path.join(modules, 'tw-animate-css')
      const manifest = JSON.parse(await readFile(path.join(dir, 'package.json'), 'utf8'))
      const style = manifest.exports?.['.']?.style ?? manifest.main ?? './dist/tw-animate.css'
      return readFile(path.join(dir, style), 'utf8')
    }
    throw new Error(`stylesheet not approved for the shell build: ${id}`)
  }
  const theme = await readFile(path.join(here, 'vendor/shadcn-admin/src/styles/theme.css'), 'utf8')
  const index = await readFile(path.join(here, 'vendor/shadcn-admin/src/styles/index.css'), 'utf8')
  const css = index.replace("@import './theme.css';", theme)
  const compiler = await compile(css, {
    async loadStylesheet(id) {
      const content = await readPackageCss(id)
      return { content, base: modules, path: id }
    },
  })
  return compiler.build(candidates)
}

function candidatesFrom(text) {
  return text.split(/[\s'"`<>{}()]+/)
}

async function sourceText() {
  const files = []
  async function walk(directory) {
    for (const entry of await readdir(directory, { withFileTypes: true })) {
      const full = path.join(directory, entry.name)
      if (entry.isDirectory()) await walk(full)
      else if (/\.(tsx|ts|css|js)$/.test(entry.name)) files.push(full)
    }
  }
  await walk(path.join(here, 'vendor'))
  await walk(path.join(here, 'compose'))
  let text = ''
  const stamp = []
  for (const file of files.sort()) {
    const bytes = await readFile(file)
    text += bytes.toString('utf8')
    stamp.push({ path: path.relative(here, file), sha256: sha256(bytes) })
  }
  return { text, stamp }
}

if (!['linux', 'win32'].includes(process.platform) || process.arch !== 'x64') {
  throw new Error('The official GUI bundle is built on Linux x64 or Windows x64')
}

const work = await mkdtemp(path.join(tmpdir(), 'winstrata-ui-'))
try {
  const lockPath = path.join(work, 'pnpm-lock.yaml')
  const npmLockPath = path.join(work, 'package-lock.json')
  curl(`https://raw.githubusercontent.com/satnaing/shadcn-admin/${shadcnRevision}/pnpm-lock.yaml`, lockPath)
  curl(`https://raw.githubusercontent.com/Subhan-code/Amicro--Micro-transitions-/${amicroRevision}/package-lock.json`, npmLockPath)
  const modules = path.join(work, 'node_modules')
  const archive = path.join(work, 'package.tgz')
  await mkdir(modules, { recursive: true })
  await unpackPnpmTree(modules, parsePnpm(await readFile(lockPath, 'utf8')), archive)
  const npm = parseNpmLock(await readFile(npmLockPath, 'utf8'))
  await unpackNpmTree(modules, npm, 'framer-motion', archive)
  const native = `@esbuild/${process.platform}-x64`
  await unpackNpmTree(modules, npm, 'esbuild', archive)
  await unpackNpmTree(modules, npm, native, archive)
  const esbuild = createRequire(path.join(modules, 'esbuild', 'package.json'))('esbuild')
  const sourceRoot = path.join(here, 'vendor/shadcn-admin/src')
  await esbuild.build({
    entryPoints: [path.join(here, 'compose/entry.tsx')],
    bundle: true,
    platform: 'browser',
    format: 'iife',
    jsx: 'automatic',
    minify: true,
    legalComments: 'eof',
    define: { 'process.env.NODE_ENV': '"production"' },
    outfile: path.join(output, 'shell.js'),
    absWorkingDir: here,
    nodePaths: [modules],
    logLevel: 'warning',
    plugins: [{
      name: 'shadcn-alias',
      setup(build) {
        build.onResolve({ filter: /^@\// }, async (args) => {
          const base = path.join(sourceRoot, args.path.slice(2))
          for (const suffix of ['.tsx', '.ts', '.js', '/index.tsx', '/index.ts']) {
            try {
              await stat(base + suffix)
              return { path: base + suffix }
            } catch {
              // 下一个后缀。
            }
          }
          return { path: base }
        })
      },
    }],
  })
  const bundled = path.join(output, 'shell.js')
  const script = (await readFile(bundled, 'utf8')).replace(/[ \t]+$/gm, '')
  await writeFile(bundled, script)
  const { text, stamp } = await sourceText()
  const css = await compileCss(modules, [...new Set(candidatesFrom(text))])
  await mkdir(output, { recursive: true })
  await writeFile(path.join(output, 'layout.css'), css)
  await copyFile(path.join(here, 'vendor/glasswork/glass.css'), path.join(output, 'glass.css'))
  await writeFile(path.join(output, 'index.html'), `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self' data: blob:; font-src 'self'; connect-src 'self'; object-src 'none'; base-uri 'none'; form-action 'none'">
<title>WinStrata</title>
<link rel="icon" href="data:,">
<link rel="stylesheet" href="./layout.css">
<link rel="stylesheet" href="./glass.css">
</head>
<body>
<div id="root"></div>
<script src="./shell.js"></script>
</body>
</html>
`)
  const notices = []
  for (const file of [
    'vendor/amicro/LICENSE',
    'vendor/glasswork/LICENSE',
    'vendor/glasswork/NOTICE',
    'vendor/shadcn-admin/LICENSE',
  ]) {
    notices.push(await readFile(path.join(here, file), 'utf8'))
  }
  await writeFile(path.join(output, 'NOTICES.txt'), notices.join('\n\n'))
  await writeFile(path.join(output, 'stamp.json'), JSON.stringify({ files: stamp }, null, 2))
  const built = await stat(path.join(output, 'shell.js'))
  console.log(`Built official GUI shell (${built.size} bytes) from the pinned upstream sources.`)
} finally {
  await rm(work, { recursive: true, force: true })
}
