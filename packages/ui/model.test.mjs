import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import { fallback, motionEnabled, sameKeys, escapeHtml } from './model.mjs';

const messages = JSON.parse(await readFile(new URL('./static/messages.json', import.meta.url), 'utf8'));

test('both locales cover the same keys and English is the fallback', () => {
  assert.equal(sameKeys(messages.en, messages['zh-CN']), true);
  assert.equal(fallback(messages, 'zh-CN', 'theme.white'), messages['zh-CN']['theme.white']);
  assert.equal(fallback(messages, 'missing', 'theme.white'), messages.en['theme.white']);
  assert.match(messages['zh-CN']['theme.white'], /\p{Script=Han}/u);
  assert.equal(messages.en['app.name'], 'WinStrata');
});

test('motion stops offscreen and when the user asks for less motion', () => {
  assert.equal(motionEnabled('visible', false), true);
  assert.equal(motionEnabled('hidden', false), false);
  assert.equal(motionEnabled('visible', true), false);
});

test('text from the host is escaped before it enters the page', () => {
  assert.equal(escapeHtml(`<a href="x">'`), '&lt;a href=&quot;x&quot;&gt;&#39;');
});
