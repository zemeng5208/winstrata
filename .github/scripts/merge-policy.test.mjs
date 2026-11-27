import assert from 'node:assert/strict';
import test from 'node:test';
import { readFileSync } from 'node:fs';

import {
  approvalText, changesPolicy, isMergeCandidate, policyDecision, roadmapOrder,
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
  blockedIssues: [], files: [{ filename: 'README.md' }], comments: [], ownerId: 42,
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

test('checks attach to the exact current test merge commit', () => {
  const commit = { parents: [{ sha: base }, { sha: head }] };
  assert.equal(isMergeCandidate(commit, pull, base), true);
  assert.equal(isMergeCandidate(commit, pull, candidate), false);
  assert.equal(isMergeCandidate({ parents: [{ sha: base }, { sha: candidate }] }, pull, base), false);
  assert.equal(isMergeCandidate(commit, { ...pull, mergeable: null }, base), false);
});
