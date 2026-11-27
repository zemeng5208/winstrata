import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';
import { setTimeout as wait } from 'node:timers/promises';

export const BLOCK_LABEL = 'merge-blocked';
const STATUS_CONTEXT = 'winstrata/merge-policy';
const SHA = /^[a-f0-9]{40}$/;

export function roadmapOrder(markdown) {
  const ids = [...markdown.matchAll(/^\| <a id="pr-(\d{3})"><\/a>\1 \|/gm)]
    .map((match) => match[1]);
  if (ids.length === 0 || new Set(ids).size !== ids.length) {
    throw new Error('Roadmap plan IDs are missing or duplicated');
  }
  return ids;
}

function planId(pull) {
  const branch = /^pr\/(\d{3})-[a-z0-9][a-z0-9-]*$/.exec(pull.head.ref);
  const title = /^PR (\d{3}):\s+\S/.exec(pull.title);
  return branch && branch[1] === title?.[1] ? branch[1] : undefined;
}

export function changesPolicy(files) {
  return files.some(({ filename, previous_filename }) =>
    [filename, previous_filename].some((path) =>
      path?.startsWith('.github/') ||
      path?.startsWith('toolchains/dev/') ||
      path === '.cargo/config.toml' ||
      path === 'AGENTS.md' ||
      path === 'docs/zh-CN/AGENTS.md' ||
      path?.startsWith('docs/standards/') ||
      path?.startsWith('docs/zh-CN/standards/') ||
      path === 'docs/roadmap/PLAN.md' ||
      path === 'docs/zh-CN/roadmap/PLAN.md',
    ),
  );
}

export function approvalText(sha) {
  return `Approve gate policy at ${sha}`;
}

export function policyDecision({ pull, order, merged, blockedIssues, files, comments, ownerId }) {
  const id = planId(pull);
  const position = order.indexOf(id);
  if (position < 0) return 'PR title and branch must identify a plan ID in the roadmap';
  if (pull.draft) return 'PR is in draft';
  if (pull.labels.some(({ name }) => name === BLOCK_LABEL)) return 'PR has a merge blocker';
  if (blockedIssues.length > 0) return `Repository has open merge-blocked issue #${blockedIssues[0].number}`;

  const completed = new Set(merged.filter(({ merged_at }) => merged_at).map(planId));
  if (completed.has(id)) return `Plan ${id} already merged`;
  const missing = order.slice(0, position).find((previous) => !completed.has(previous));
  if (missing) return `Plan ${missing} must merge before plan ${id}`;

  if (files.length !== pull.changed_files) return 'The complete PR file list could not be verified';
  if (changesPolicy(files) && !comments.some(({ user, body }) =>
    user?.id === ownerId && body?.trim() === approvalText(pull.head.sha),
  )) return 'Policy changes require owner confirmation for this exact head SHA';
  return null;
}

export function isMergeCandidate(commit, pull, main) {
  return pull.state === 'open' && pull.base.ref === 'main' && pull.base.sha === main &&
    pull.mergeable === true && SHA.test(pull.merge_commit_sha) &&
    commit.parents.length === 2 &&
    commit.parents[0].sha === main && commit.parents[1].sha === pull.head.sha;
}

function github(repo, token, apiRoot) {
  return async (path, body) => {
    const suffix = path ? `/${path}` : '';
    const response = await fetch(`${apiRoot}/repos/${repo}${suffix}`, {
      method: body ? 'POST' : 'GET',
      headers: {
        accept: 'application/vnd.github+json',
        authorization: `Bearer ${token}`,
        'content-type': 'application/json',
        'x-github-api-version': '2022-11-28',
        'user-agent': 'winstrata-merge-policy',
      },
      body: body && JSON.stringify(body),
      redirect: 'error',
      signal: AbortSignal.timeout(15_000),
    });
    if (!response.ok) {
      const error = new Error(`GitHub ${response.status} for ${path}`);
      error.status = response.status;
      throw error;
    }
    return response.json();
  };
}

async function list(api, path) {
  const pages = [];
  const join = path.includes('?') ? '&' : '?';
  for (let page = 1; ; page += 1) {
    const batch = await api(`${path}${join}per_page=100&page=${page}`);
    pages.push(...batch);
    if (batch.length < 100) return pages;
  }
}

async function currentCandidate(api, number, base) {
  // GitHub 在请求 PR 后异步生成测试合并提交。仅接受父提交与当前 PR 相符的候选。
  for (let attempt = 0; attempt < 10; attempt += 1) {
    const pull = await api(`pulls/${number}`);
    if (pull.state !== 'open' || pull.base.ref !== 'main' || pull.mergeable === false) return null;
    if (pull.mergeable === true && SHA.test(pull.merge_commit_sha)) {
      try {
        const commit = await api(`git/commits/${pull.merge_commit_sha}`);
        if (isMergeCandidate(commit, pull, base)) return pull;
      } catch (error) {
        if (error.status !== 404) throw error;
      }
    }
    if (attempt < 9) await wait(2_000);
  }
  throw new Error(`Could not verify the test merge commit for PR #${number}`);
}

async function main() {
  const { GITHUB_REPOSITORY: repo, GITHUB_TOKEN: token, GITHUB_EVENT_NAME: eventName,
    GITHUB_EVENT_PATH: eventFile, GITHUB_RUN_ID: runId } = process.env;
  if (!repo || !token || !eventFile || !runId) throw new Error('Missing GitHub Actions environment');
  const event = JSON.parse(readFileSync(eventFile, 'utf8'));
  if (eventName === 'issues' && event.label?.name !== BLOCK_LABEL &&
      !event.issue.labels.some(({ name }) => name === BLOCK_LABEL)) return;
  if (eventName === 'issue_comment' && (
    !event.issue.pull_request || event.comment.user?.id !== event.repository.owner.id
  )) return;

  const api = github(repo, token, process.env.GITHUB_API_URL ?? 'https://api.github.com');
  const targetUrl = `${process.env.GITHUB_SERVER_URL ?? 'https://github.com'}/${repo}/actions/runs/${runId}`;
  const base = execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim();
  const branch = await api('branches/main');
  if (branch.commit.sha !== base) throw new Error('Main changed during policy checkout; rerun');
  const order = roadmapOrder(readFileSync('docs/roadmap/PLAN.md', 'utf8'));
  const open = await list(api, 'pulls?state=open&base=main');
  let related = null;
  if (eventName === 'pull_request_target') related = event.pull_request;
  if (eventName === 'issue_comment') {
    related = open.find(({ number }) => number === event.issue.number);
    if (!related) return;
  }
  const selected = related
    ? open.filter(({ number, head }) => number === related.number || head.sha === related.head.sha)
    : open;
  const candidates = [];
  let errors = 0;
  const status = (sha, state, description) => api(`statuses/${sha}`, {
    state, description, context: STATUS_CONTEXT, target_url: targetUrl,
  });

  for (const { number } of selected) {
    try {
      const pull = await currentCandidate(api, number, base);
      if (!pull) continue;
      await status(pull.merge_commit_sha, 'pending', 'Checking merge order and blockers');
      candidates.push(pull);
    } catch (error) {
      errors += 1;
      console.error(`PR #${number}: ${error.message}`);
    }
  }

  if (candidates.length > 0) {
    const [merged, issues, repository] = await Promise.all([
      list(api, 'pulls?state=closed&base=main'),
      list(api, `issues?state=open&labels=${BLOCK_LABEL}`),
      api(''),
    ]);
    const blockedIssues = issues.filter((item) => !item.pull_request);
    for (const pull of candidates) {
      try {
        const files = await list(api, `pulls/${pull.number}/files`);
        const comments = changesPolicy(files)
          ? await list(api, `issues/${pull.number}/comments`)
          : [];
        const shared = candidates.some((other) =>
          other.number !== pull.number && other.merge_commit_sha === pull.merge_commit_sha,
        );
        const failure = shared ? 'Multiple PRs share this test merge commit' : policyDecision({
          pull, order, merged, blockedIssues, files, comments, ownerId: repository.owner.id,
        });
        const latest = await api(`pulls/${pull.number}`);
        if (latest.state !== 'open' || latest.head.sha !== pull.head.sha ||
            latest.base.sha !== base || latest.merge_commit_sha !== pull.merge_commit_sha) {
          console.log(`PR #${pull.number}: candidate changed; waiting for the next run`);
          continue;
        }
        await status(pull.merge_commit_sha, failure ? 'failure' : 'success',
          failure ?? 'Merge order, blockers, and policy files checked');
        console.log(`PR #${pull.number}: ${failure ?? 'merge policy passed'}`);
      } catch (error) {
        errors += 1;
        console.error(`PR #${pull.number}: ${error.message}`);
      }
    }
  }
  if (errors > 0) throw new Error('Merge policy could not verify all candidates');
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((error) => {
    console.error(error.message);
    process.exitCode = 1;
  });
}
