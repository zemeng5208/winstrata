import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const pin = JSON.parse(await readFile(new URL('./upstream.json', import.meta.url), 'utf8'));

test('vendored upstream files match the pinned bytes', async () => {
  assert.equal(pin.sources.length, 3);
  for (const source of pin.sources) {
    assert.equal(source.license, 'MIT');
    assert.match(source.revision, /^[0-9a-f]{40}$/);
    for (const file of source.files) {
      const bytes = await readFile(path.join(root, file.path));
      assert.equal(createHash('sha256').update(bytes).digest('hex'), file.sha256);
    }
  }
  const notices = await readFile(path.join(root, 'packages/ui/vendor/glasswork/NOTICE'), 'utf8');
  assert.match(notices, /do not reuse or redistribute them/);
  assert.equal(pin.bundles.amicro.react.version, '19.2.7');
  assert.equal(pin.bundles.shadcn.react.version, '19.2.5');
  assert.equal(pin.bundles.amicro.esbuild.version, '0.25.12');
  const roles = new Set(pin.surfaces.map((surface) => surface.role));
  assert.deepEqual([...roles].sort(), ['host-adapter', 'not-included', 'official-component', 'paused-host-authored']);
  for (const surface of pin.surfaces) {
    for (const field of ['id', 'role', 'license', 'reuse', 'modifications']) {
      assert.equal(typeof surface[field], 'string');
      assert.notEqual(surface[field], '');
    }
    if (surface.role === 'official-component') {
      assert.equal(surface.reuse, 'verbatim-file');
      assert.match(surface.revision, /^[0-9a-f]{40}$/);
      const bytes = await readFile(path.join(root, surface.vendored));
      assert.ok(bytes.length > 0);
    }
  }
  assert.equal(pin.surfaces.find((surface) => surface.id === 'ui-moments').reuse, 'not-included');
});
