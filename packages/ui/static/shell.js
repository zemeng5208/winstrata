import { escapeHtml, fallback, motionEnabled } from './model.mjs';

const token = '';
globalThis.WINSTRATA_PROBE = new URLSearchParams(location.search).get('probe') === '1';
let messages = null;
let state = null;
let amicro = null;
let lens = null;
let shadcnMounted = false;

async function api(method, action, value, remember) {
  const response = await fetch(method === 'GET' ? 'api/state' : 'api/command', {
    method,
    headers: {
      'content-type': 'application/json',
      'x-winstrata-token': token || '',
    },
    body: method === 'GET' ? undefined : JSON.stringify({ action, value, remember: Boolean(remember) }),
  });
  if (!response.ok) throw new Error(`host returned ${response.status}`);
  return response.json();
}

function text(key) {
  return fallback(messages, state?.language ?? 'en', key);
}

function themeName(theme) {
  if (theme === 'white-acrylic') return text('theme.white');
  if (theme === 'black-acrylic') return text('theme.black');
  if (theme === 'liquid-glass') return text('theme.liquid');
  return text('theme.choose');
}

function applyTheme() {
  document.documentElement.lang = state.language === 'zh-CN' ? 'zh-CN' : 'en';
  document.querySelector('#app').dataset.theme = state.theme ?? 'unset';
  const button = document.querySelector('#glass-action');
  if (lens) {
    lens.destroy();
    lens = null;
  }
  if (button && state.theme === 'liquid-glass' && motionEnabled(document.visibilityState, reduced())) {
    button.classList.add('glass');
    lens = globalThis.mountGlass(button);
  } else if (button) {
    button.classList.remove('glass');
  }
}

function reduced() {
  return window.matchMedia('(prefers-reduced-motion: reduce)').matches;
}

function pageHtml() {
  const plugin = state.plugins.find((item) => `plugin:${item.id}` === state.page);
  if (state.page === 'home') {
    const items = state.plugins.map((item) =>
      `<li><button class="nav-button" data-page="plugin:${escapeHtml(item.id)}">${escapeHtml(item.name)}</button> <span class="muted">${escapeHtml(item.availability)}</span></li>`).join('');
    return `<h1>${escapeHtml(text('app.name'))}</h1><p>${escapeHtml(text('home.lead'))}</p><p>${escapeHtml(text(state.pluginHost === 'host' ? 'plugin.host' : 'plugin.catalog'))}</p><ul>${items}</ul>`;
  }
  if (state.page === 'settings') {
    const themes = ['white-acrylic', 'black-acrylic', 'liquid-glass'].map((theme) =>
      `<button class="choice" data-theme="${theme}">${escapeHtml(themeName(theme))}</button>`).join('');
    const languages = `<button class="choice" data-language="en">${escapeHtml(text('language.en'))}</button><button class="choice" data-language="zh-CN">${escapeHtml(text('language.zh'))}</button>`;
    const closes = ['ask', 'tray', 'exit'].map((close) =>
      `<button class="choice" data-close="${close}">${escapeHtml(text(`close.${close}`))}</button>`).join('');
    const drivers = state.service.state === 'ready'
      ? `<ul>${state.service.drivers.map((driver) => `<li>${escapeHtml(driver.driver)}: ${escapeHtml(driver.phase)}${driver.failure ? ` (${escapeHtml(driver.failure)})` : ''}; ${escapeHtml(text('driver.subscriptions'))} ${driver.subscriptions}</li>`).join('')}</ul>`
      : `<p>${escapeHtml(state.service.detail ?? text('service.unavailable'))}</p>`;
    return `<h1>${escapeHtml(text('nav.settings'))}</h1>
      <section class="panel"><h2>${escapeHtml(text('theme.choose'))}</h2><div class="row">${themes}</div></section>
      <section class="panel"><h2>${escapeHtml(text('language.label'))}</h2><div class="row">${languages}</div></section>
      <section class="panel"><h2>${escapeHtml(text('settings.close'))}</h2><div class="row">${closes}</div><p>${escapeHtml(text('close.trayUnavailable'))}</p></section>
      <section class="panel"><h2>${escapeHtml(text('storage.label'))}</h2><p>${escapeHtml(storageLine())}</p>${state.error ? `<p>${escapeHtml(state.error)}</p>` : ''}<form id="storage"><input name="path" autocomplete="off" spellcheck="false"><button type="submit">${escapeHtml(text('storage.save'))}</button></form></section>
      <section class="panel"><h2>${escapeHtml(text('service.ready'))}</h2>${drivers}<p>${escapeHtml(privilegeLine())}</p><p>${escapeHtml(text(state.tray === 'ready' ? 'tray.ready' : 'tray.unsupported'))}</p></section>`;
  }
  if (state.page === 'market') return `<h1>${escapeHtml(text('nav.market'))}</h1><p>${escapeHtml(text('market.body'))}</p>`;
  if (state.page === 'onboarding') return `<h1>${escapeHtml(text('nav.onboarding'))}</h1><p>${escapeHtml(text('onboarding.body'))}</p>`;
  if (plugin) {
    return `<h1>${escapeHtml(plugin.name)}</h1><p>${escapeHtml(plugin.id)}</p><p>${escapeHtml(text('plugin.absent'))}</p><p>${escapeHtml(plugin.phase)}</p><p>${escapeHtml(String(plugin.plan ?? ''))}</p>`;
  }
  return `<h1>${escapeHtml(text('app.name'))}</h1><p>${escapeHtml(text('bundle.missing'))}</p>`;
}

function storageLine() {
  if (state.storage.state === 'ready') return `${text('storage.ready')} ${state.storage.root ?? ''} ${text(`run.${state.storage.run}`)}`;
  if (state.storage.state === 'needs_directory') return `${text('storage.needs')} ${state.storage.detail ?? ''}`;
  return `${text('storage.unavailable')} ${state.storage.detail ?? ''}`;
}

function privilegeLine() {
  if (state.privilege === 'windows_ordinary') return text('privilege.ordinary');
  if (state.privilege === 'windows_elevated') return text('privilege.elevated');
  if (state.privilege === 'not_windows') return text('privilege.not_windows');
  return text('privilege.unknown');
}

function renderChrome() {
  const nav = [
    ['home', 'nav.home'],
    ['settings', 'nav.settings'],
    ['market', 'nav.market'],
    ['onboarding', 'nav.onboarding'],
  ].map(([page, key]) => `<button class="nav-button" data-page="${page}" ${state.page === page ? 'aria-current="page"' : ''}>${escapeHtml(text(key))}</button>`).join('');
  const plugins = state.plugins.map((item) =>
    `<button class="nav-button" data-page="plugin:${escapeHtml(item.id)}" ${state.page === `plugin:${item.id}` ? 'aria-current="page"' : ''}>${escapeHtml(item.name)}</button>`).join('');
  document.querySelector('#nav').innerHTML = `<strong>${escapeHtml(text('app.name'))}</strong>${nav}<h2>${escapeHtml(text('nav.plugins'))}</h2>${plugins}`;
  document.querySelector('#top').innerHTML = `<span>${escapeHtml(themeName(state.theme))}</span><button id="glass-action" type="button" class="text-button">${escapeHtml(text('close.title'))}</button><a href="NOTICES.txt">${escapeHtml(text('notices'))}</a>`;
  const chooser = document.querySelector('#chooser');
  chooser.hidden = state.theme != null;
  chooser.innerHTML = `<section><h2>${escapeHtml(text('theme.choose'))}</h2><p>${escapeHtml(text('theme.proposal'))}</p><div class="row">${['white-acrylic', 'black-acrylic', 'liquid-glass'].map((theme) => `<button class="choice" data-theme="${theme}">${escapeHtml(themeName(theme))}</button>`).join('')}</div></section>`;
  if (!state.bundle) {
    document.querySelector('#shadcn').innerHTML = `<main data-layout="fixed"><div id="page">${pageHtml()}</div></main>`;
  }
}

function renderPage() {
  renderChrome();
  const enabled = motionEnabled(document.visibilityState, reduced());
  if (state.bundle && typeof globalThis.mountShadcn === 'function' && typeof globalThis.mountAmicro === 'function') {
    if (!shadcnMounted) {
      const slot = globalThis.mountShadcn(document.querySelector('#shadcn'), true, false);
      amicro = globalThis.mountAmicro(slot, pageHtml(), enabled);
      shadcnMounted = true;
    } else if (amicro) {
      amicro.render(pageHtml(), enabled);
    }
  }
  applyTheme();
  bind();
}

function bind() {
  document.querySelectorAll('[data-page]').forEach((node) => {
    node.onclick = () => api('POST', 'navigate', node.dataset.page).then(applyState);
  });
  document.querySelectorAll('[data-theme]').forEach((node) => {
    node.onclick = () => api('POST', 'set_theme', node.dataset.theme).then(applyState);
  });
  document.querySelectorAll('[data-language]').forEach((node) => {
    node.onclick = () => api('POST', 'set_language', node.dataset.language).then(applyState);
  });
  document.querySelectorAll('[data-close]').forEach((node) => {
    node.onclick = () => api('POST', 'set_close', node.dataset.close).then(applyState);
  });
  const storage = document.querySelector('#storage');
  if (storage) {
    storage.onsubmit = (event) => {
      event.preventDefault();
      api('POST', 'set_storage', new FormData(storage).get('path')).then(applyState);
    };
  }
  const close = document.querySelector('#glass-action');
  if (close) close.onclick = () => { document.querySelector('#close-dialog').hidden = false; };
  const dialog = document.querySelector('#close-dialog');
  const trayDisabled = state.tray !== 'ready';
  dialog.innerHTML = `<section><h2>${escapeHtml(text('close.title'))}</h2><p>${escapeHtml(text('close.body'))}</p><label><input id="remember" type="checkbox"> ${escapeHtml(text('close.remember'))}</label><div class="row"><button id="choose-tray" type="button" ${trayDisabled ? 'disabled' : ''}>${escapeHtml(text('close.tray'))}</button><button id="choose-exit" type="button">${escapeHtml(text('close.exit'))}</button><button id="choose-cancel" type="button">${escapeHtml(text('close.cancel'))}</button></div>${trayDisabled ? `<p>${escapeHtml(text('close.trayUnavailable'))}</p>` : ''}</section>`;
  document.querySelector('#choose-cancel').onclick = () => { dialog.hidden = true; };
  document.querySelector('#choose-tray').onclick = () => api('POST', 'choose_close', 'tray', document.querySelector('#remember').checked).then(applyState);
  document.querySelector('#choose-exit').onclick = () => api('POST', 'choose_close', 'exit', document.querySelector('#remember').checked);
}

function suspendMotion(visibility) {
  if (!motionEnabled(visibility, reduced())) {
    if (lens) {
      lens.destroy();
      lens = null;
    }
    if (amicro) amicro.render(pageHtml(), false);
    return false;
  }
  applyTheme();
  if (amicro) amicro.render(pageHtml(), true);
  return true;
}

function applyState(next) {
  state = next;
  renderPage();
  return state;
}

async function probe() {
  const report = { ok: false, checks: {}, timingsMs: {} };
  const clock = () => performance.now();
  const frame = () => new Promise((resolve) => requestAnimationFrame(resolve));
  const check = (name, value) => {
    report.checks[name] = Boolean(value);
    if (!report.checks[name]) throw new Error(name);
  };
  try {
    await api('POST', 'set_theme', 'white-acrylic');
    await api('POST', 'set_theme', 'black-acrylic');
    state = await api('POST', 'set_theme', 'liquid-glass');
    renderPage();
    check('themes', document.querySelector('#app').dataset.theme === 'liquid-glass' && state.theme === 'liquid-glass');
    const persisted = await api('GET');
    check('themePersisted', persisted.theme === 'liquid-glass');
    state = await api('POST', 'set_language', 'zh-CN');
    renderPage();
    check('language', document.body.textContent.includes(messages['zh-CN']['nav.settings']));
    state = await api('POST', 'set_language', 'en');
    renderPage();
    const container = document.querySelector('#shadcn');
    container.style.width = '1440px';
    const main = container.querySelector('main');
    const bounded = main.getBoundingClientRect().width;
    const style = getComputedStyle(main);
    check('shadcnLayout', main.dataset.layout === 'fixed' && style.display === 'flex' && Number.parseFloat(style.paddingLeft) > 0 && bounded > 0 && bounded < container.clientWidth);
    container.style.width = '640px';
    await frame();
    const narrowed = main.getBoundingClientRect().width;
    check('shadcnResize', narrowed < bounded && Math.abs(narrowed - container.clientWidth) <= 1);
    const button = document.querySelector('#glass-action');
    const started = clock();
    if (lens) lens.destroy();
    lens = globalThis.mountGlass(button);
    const glassReady = clock() - started;
    report.timingsMs.glassReady = glassReady;
    const filter = getComputedStyle(button).backdropFilter || getComputedStyle(button).webkitBackdropFilter || '';
    check('glassRefraction', lens.supported && lens.bezel > 0 && filter.includes('blur(') && document.querySelector('feDisplacementMap'));
    check('motionSuspended', suspendMotion('hidden') === false && !button.style.getPropertyValue('--glass-lens'));
    let last = await frame();
    const intervals = [];
    while (intervals.length < 30) {
      const next = await frame();
      intervals.push(next - last);
      last = next;
    }
    intervals.sort((a, b) => a - b);
    report.frameSamples = intervals.length;
    report.frameMedianMs = (intervals[14] + intervals[15]) / 2;
    report.frameP95Ms = intervals[28];
    report.memory = performance.memory ? { usedJsHeapSize: performance.memory.usedJsHeapSize } : null;
    const notices = await fetch('NOTICES.txt').then((response) => response.text());
    check('licenses', notices.includes('MIT') && notices.includes('Amicro') && notices.includes('Glasswork') && notices.includes('shadcn-admin'));
    report.ok = true;
  } catch (error) {
    report.error = String(error);
  }
  globalThis.probeReport = report;
}

document.addEventListener('visibilitychange', () => suspendMotion(document.visibilityState));
globalThis.suspendMotion = suspendMotion;

messages = await fetch('messages.json').then((response) => response.json());
state = await api('GET');
renderPage();
if (globalThis.WINSTRATA_PROBE) probe();
