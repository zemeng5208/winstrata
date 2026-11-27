import assert from 'node:assert/strict';
import { execFileSync, spawn } from 'node:child_process';
import test from 'node:test';
import { readFileSync } from 'node:fs';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import {
  approvalText, changesPolicy, isMergeCandidate, policyDecision, roadmapExtension, roadmapOrder,
} from './merge-policy.mjs';

const base = 'a'.repeat(40);
const head = 'b'.repeat(40);
const candidate = 'c'.repeat(40);
const repoPlan = roadmapOrder(readFileSync('docs/roadmap/PLAN.md', 'utf8'));
const accepted = (id) => ({
  title: `PR ${id}: Earlier change`,
  head: { ref: `pr/${id}-earlier-change` },
  merged_at: '2026-11-27T07:00:00Z',
});
const pull = {
  title: 'PR 002: Merge policy',
  state: 'open',
  draft: false,
  head: { ref: 'pr/002-merge-gates', sha: head },
  base: { ref: 'main', sha: base },
  mergeable: true,
  merge_commit_sha: candidate,
  labels: [],
  changed_files: 1,
};
const input = {
  pull, order: repoPlan, merged: [accepted('000'), accepted('001')],
  blockedIssues: [], files: [{ filename: 'README.md' }], comments: [], ownerId: 42, upToDate: true,
};

test('real roadmap has ordered plan IDs and detects duplicate IDs', () => {
  assert.deepEqual(repoPlan.slice(0, 4), ['000', '001', '002', '003']);
  assert.throws(() => roadmapOrder('| <a id="pr-002"></a>002 |\n| <a id="pr-002"></a>002 |'));
});

test('ordered PR without outstanding blockers passes', () => {
  assert.equal(policyDecision(input), null);
});

test('unmerged, missing or duplicate plan numbers block progression', () => {
  assert.match(policyDecision({ ...input, merged: [accepted('000')] }), /001 must merge/);
  assert.match(policyDecision({
    ...input, merged: [...input.merged, { ...accepted('002'), merged_at: null }],
    pull: { ...pull, title: 'PR 003: Next', head: { ...pull.head, ref: 'pr/003-next' } },
  }), /002 must merge/);
  assert.match(policyDecision({ ...input, merged: [...input.merged, accepted('002')] }), /already merged/);
  assert.match(policyDecision({ ...input, pull: { ...pull, title: 'PR 003: Wrong ID' } }), /identify a plan ID/);
});

test('drafts and PR or repository defect blockers fail closed', () => {
  assert.match(policyDecision({ ...input, pull: { ...pull, draft: true } }), /draft/);
  assert.match(policyDecision({ ...input, pull: { ...pull, labels: [{ name: 'merge-blocked' }] } }), /merge blocker/);
  assert.match(policyDecision({ ...input, blockedIssues: [{ number: 17 }] }), /#17/);
});

test('governance changes need owner approval for the current commit', () => {
  const modified = { ...input, files: [{ filename: '.github/workflows/workspace-checks.yml' }] };
  const comment = { user: { id: 42 }, body: approvalText(head) };
  assert.equal(changesPolicy([{ filename: 'README.md', previous_filename: '.github/scripts/check.mjs' }]), true);
  assert.match(policyDecision(modified), /require owner confirmation/);
  assert.match(policyDecision({ ...modified, comments: [{ ...comment, user: { id: 99 } }] }), /require owner/);
  assert.match(policyDecision({ ...modified, comments: [{ ...comment, body: approvalText(base) }] }), /require owner/);
  assert.equal(policyDecision({ ...modified, comments: [comment] }), null);
  assert.match(policyDecision({ ...input, files: [] }), /complete PR file list/);
});

test('an added plan is accepted only as an insertion into the main order', () => {
  const headOrder = roadmapOrder(readFileSync('docs/roadmap/PLAN.md', 'utf8'));
  const baseOrder = headOrder.filter((id) => id !== '052');
  assert.deepEqual(
    baseOrder.slice(baseOrder.indexOf('013'), baseOrder.indexOf('015') + 1),
    ['013', '014', '015'],
  );
  assert.equal(roadmapExtension(baseOrder, headOrder), null);
  assert.equal(headOrder[headOrder.indexOf('013') + 1], '052');
  assert.equal(headOrder[headOrder.indexOf('052') + 1], '014');

  const planFiles = [{ filename: 'docs/roadmap/PLAN.md' }];
  const comment = { user: { id: 42 }, body: approvalText(head) };
  const merged = baseOrder.slice(0, baseOrder.indexOf('013') + 1).map(accepted);
  const pull052 = {
    ...pull,
    title: 'PR 052: Process exit sample',
    head: { ref: 'pr/052-process-exit-sample', sha: head },
  };
  const added = {
    pull: pull052,
    order: headOrder,
    merged,
    blockedIssues: [],
    files: planFiles,
    comments: [comment],
    ownerId: 42,
    upToDate: true,
  };
  assert.match(policyDecision({ ...added, comments: [] }), /require owner confirmation/);
  assert.match(policyDecision({ ...added, upToDate: false }), /current main commit/);
  assert.match(policyDecision({ ...added, pull: { ...pull052, draft: true } }), /draft/);
  assert.match(policyDecision({
    ...added,
    pull: { ...pull052, labels: [{ name: 'merge-blocked' }] },
  }), /merge blocker/);
  assert.match(policyDecision({ ...added, blockedIssues: [{ number: 17 }] }), /#17/);
  assert.match(policyDecision({
    ...added,
    pull: { ...pull052, title: 'PR 014: Wrong title' },
  }), /identify a plan ID/);
  assert.equal(policyDecision(added), null);
  assert.match(policyDecision({ ...added, order: baseOrder }), /identify a plan ID/);
  assert.match(policyDecision({
    ...added,
    pull: { ...pull052, title: 'PR 014: Plugins', head: { ref: 'pr/014-plugins', sha: head } },
  }), /052 must merge before plan 014/);
  assert.match(policyDecision({
    ...added,
    pull: { ...pull052, title: 'PR 013: Again', head: { ref: 'pr/013-again', sha: head } },
    merged: baseOrder.map(accepted),
  }), /Plan 013 already merged/);
  assert.match(roadmapExtension(baseOrder, headOrder.filter((id) => id !== '014')), /drops plan 014/);
  const swapped = headOrder.slice();
  const at = swapped.indexOf('014');
  [swapped[at], swapped[at + 1]] = [swapped[at + 1], swapped[at]];
  assert.match(roadmapExtension(baseOrder, swapped), /reorders plan 015/);
  const tail = headOrder.slice();
  const from = tail.indexOf('014');
  const to = tail.indexOf('050');
  [tail[from], tail[to]] = [tail[to], tail[from]];
  assert.match(roadmapExtension(baseOrder, tail), /reorders plan 050/);
});

test('policy verifies the actual merge candidate and rejects a stale base', () => {
  const commit = { parents: [{ sha: base }, { sha: head }] };
  assert.equal(isMergeCandidate(commit, pull, base), true);
  assert.equal(isMergeCandidate(commit, pull, candidate), false);
  assert.equal(isMergeCandidate({ parents: [{ sha: base }, { sha: candidate }] }, pull, base), false);
  assert.equal(isMergeCandidate(commit, { ...pull, mergeable: null }, base), false);
  assert.match(policyDecision({ ...input, upToDate: false }), /current main commit/);
});

test('policy posts acceptance and blockers on the same head as the workspace check', async () => {
  const actualBase = execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim();
  const current = { ...pull, number: 9, base: { ref: 'main', sha: actualBase } };
  const posts = [];
  let hasIssue = false;

  const github = createServer(async (request, response) => {
    const url = new URL(request.url, 'http://localhost');
    const path = url.pathname.replace(/^\/repos\/test\/winstrata/, '');
    const routes = {
      '': { owner: { id: input.ownerId } },
      '/branches/main': { commit: { sha: actualBase } },
      '/pulls': url.searchParams.get('state') === 'closed' ? input.merged : [current],
      '/pulls/9': current,
      '/pulls/9/files': input.files,
      '/issues': hasIssue ? [{ number: 90 }] : [],
      [`/compare/${actualBase}...${head}`]: { merge_base_commit: { sha: actualBase } },
      [`/git/commits/${candidate}`]: { parents: [{ sha: actualBase }, { sha: head }] },
    };
    if (request.method === 'POST') {
      let body = '';
      for await (const chunk of request) body += chunk;
      posts.push([path, JSON.parse(body)]);
      response.writeHead(201, { 'content-type': 'application/json' }).end('{}');
    } else if (Object.hasOwn(routes, path)) {
      response.writeHead(200, { 'content-type': 'application/json' }).end(JSON.stringify(routes[path]));
    } else {
      response.writeHead(404).end();
    }
  });

  const directory = await mkdtemp(join(tmpdir(), 'winstrata-merge-policy.'));
  try {
    const eventPath = join(directory, 'event.json');
    await writeFile(eventPath, JSON.stringify({ pull_request: current }));
    await new Promise((resolve, reject) => {
      github.once('error', reject);
      github.listen(0, '127.0.0.1', resolve);
    });
    for (const blocked of [false, true]) {
      hasIssue = blocked;
      posts.length = 0;
      const child = spawn(process.execPath, ['.github/scripts/merge-policy.mjs'], {
        stdio: 'inherit',
        env: {
          ...process.env,
          GITHUB_API_URL: `http://127.0.0.1:${github.address().port}`,
          GITHUB_EVENT_NAME: 'pull_request_target',
          GITHUB_EVENT_PATH: eventPath,
          GITHUB_REPOSITORY: 'test/winstrata',
          GITHUB_RUN_ID: '1',
          GITHUB_SERVER_URL: 'https://example.test',
          GITHUB_TOKEN: 'fixture-token',
        },
      });
      const result = await new Promise((resolve, reject) => {
        child.once('error', reject);
        child.once('close', resolve);
      });
      assert.equal(result, 0);
      assert.deepEqual(posts.map(([path, data]) => [path, data.context, data.state]), [
        [`/statuses/${head}`, 'winstrata/merge-policy', 'pending'],
        [`/statuses/${head}`, 'winstrata/merge-policy', blocked ? 'failure' : 'success'],
      ]);
    }
  } finally {
    github.close();
    await rm(directory, { recursive: true, force: true });
  }
});

test('policy reads an added plan from the test merge and rejects a damaged roadmap', async () => {
  const actualBase = execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim();
  const disk = readFileSync('docs/roadmap/PLAN.md', 'utf8');
  const diskOrder = roadmapOrder(disk);
  const row052 = disk.match(/^\| <a id="pr-052"><\/a>052 \|.*$/m);
  assert.ok(row052);
  const added = disk.replace(row052[0], `${row052[0]}\n| <a id="pr-099"></a>099 | fixture | 052 | Fixture. |`);
  const dropped = disk.split('\n').filter((line) => !line.includes('id="pr-014"')).join('\n');
  const duplicated = disk.replace(row052[0], `${row052[0]}\n${row052[0]}`);
  const merged = diskOrder.slice(0, diskOrder.indexOf('052')).map(accepted);
  const sha = 'd'.repeat(40);
  const merge = 'e'.repeat(40);
  const bodies = { add: added, unapproved: added, drop: dropped, duplicate: duplicated };
  let mode = 'add';
  const posts = [];
  const current = {
    title: 'PR 052: Process exit sample',
    state: 'open',
    draft: false,
    number: 17,
    head: { ref: 'pr/052-process-exit-sample', sha },
    base: { ref: 'main', sha: actualBase },
    mergeable: true,
    merge_commit_sha: merge,
    labels: [],
    changed_files: 1,
  };
  const github = createServer(async (request, response) => {
    const url = new URL(request.url, 'http://localhost');
    const path = url.pathname.replace(/^\/repos\/test\/winstrata/, '');
    const plan = bodies[mode];
    const routes = {
      '': { owner: { id: 42 } },
      '/branches/main': { commit: { sha: actualBase } },
      '/pulls': url.searchParams.get('state') === 'closed' ? merged : [current],
      '/pulls/17': current,
      '/pulls/17/files': [{ filename: 'docs/roadmap/PLAN.md' }],
      '/issues/17/comments': mode === 'unapproved' ? [] : [{ user: { id: 42 }, body: approvalText(sha) }],
      '/issues': [],
      [`/compare/${actualBase}...${sha}`]: { merge_base_commit: { sha: actualBase } },
      [`/git/commits/${merge}`]: { parents: [{ sha: actualBase }, { sha }] },
      '/contents/docs/roadmap/PLAN.md': {
        encoding: 'base64',
        content: Buffer.from(plan, 'utf8').toString('base64'),
      },
    };
    if (url.searchParams.get('ref') && url.searchParams.get('ref') !== merge) {
      response.writeHead(404).end();
      return;
    }
    if (request.method === 'POST') {
      let body = '';
      for await (const chunk of request) body += chunk;
      posts.push(JSON.parse(body));
      response.writeHead(201, { 'content-type': 'application/json' }).end('{}');
    } else if (Object.hasOwn(routes, path)) {
      response.writeHead(200, { 'content-type': 'application/json' }).end(JSON.stringify(routes[path]));
    } else {
      response.writeHead(404).end();
    }
  });
  const directory = await mkdtemp(join(tmpdir(), 'winstrata-merge-policy-plan.'));
  try {
    const eventPath = join(directory, 'event.json');
    await writeFile(eventPath, JSON.stringify({ pull_request: current }));
    await new Promise((resolve, reject) => {
      github.once('error', reject);
      github.listen(0, '127.0.0.1', resolve);
    });
    const run = async (next, pull) => {
      mode = next;
      Object.assign(current, pull);
      posts.length = 0;
      const child = spawn(process.execPath, ['.github/scripts/merge-policy.mjs'], {
        stdio: 'inherit',
        env: {
          ...process.env,
          GITHUB_API_URL: `http://127.0.0.1:${github.address().port}`,
          GITHUB_EVENT_NAME: 'pull_request_target',
          GITHUB_EVENT_PATH: eventPath,
          GITHUB_REPOSITORY: 'test/winstrata',
          GITHUB_RUN_ID: '1',
          GITHUB_SERVER_URL: 'https://example.test',
          GITHUB_TOKEN: 'fixture-token',
        },
      });
      const result = await new Promise((resolve, reject) => {
        child.once('error', reject);
        child.once('close', resolve);
      });
      assert.equal(result, 0);
      return posts.at(-1);
    };
    const unapproved = await run('unapproved', {});
    assert.equal(unapproved.state, 'failure');
    assert.match(unapproved.description, /require owner confirmation/);
    const inserted = await run('add', {});
    assert.equal(inserted.state, 'success');
    assert.equal(inserted.description, 'Merge order, blockers, and policy files checked');
    const removed = await run('drop', {});
    assert.equal(removed.state, 'failure');
    assert.match(removed.description, /drops plan 014/);
    const repeated = await run('duplicate', {});
    assert.equal(repeated.state, 'failure');
    assert.match(repeated.description, /missing or duplicated/);
    const predecessor = await run('add', {
      title: 'PR 099: Later plan',
      head: { ref: 'pr/099-later-plan', sha },
    });
    assert.equal(predecessor.state, 'failure');
    assert.match(predecessor.description, /052 must merge before plan 099/);
  } finally {
    github.close();
    await rm(directory, { recursive: true, force: true });
  }
});
