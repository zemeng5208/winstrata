(async () => {
  const checks = {};
  const timingsMs = {};
  const report = { ok: false, checks, timingsMs };
  const clock = () => performance.now();
  const frame = () => new Promise(resolve => requestAnimationFrame(resolve));

  function check(name, value) {
    checks[name] = Boolean(value);
    if (!checks[name]) throw Error(name);
  }

  async function until(predicate, name) {
    const start = clock();
    while (!predicate()) {
      if (clock() - start > 8000) throw Error(name + ' timed out');
      await frame();
    }
    return clock() - start;
  }

  async function measureFrames() {
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
  }

  try {
    const frames = measureFrames();
    let start = clock();
    const unmount = globalThis.mountAmicro();
    const transition = document.querySelector('#amicro > div');
    const opacity = () => Number(getComputedStyle(transition).opacity);
    const initialOpacity = opacity();
    let intermediate = false;
    await until(() => {
      const current = opacity();
      if (current > 0.001 && current < 0.999) intermediate = true;
      return current >= 0.999;
    }, 'Amicro');
    timingsMs.amicroTransition = clock() - start;
    check('amicroTransition', initialOpacity <= 0.01 && intermediate && transition.textContent === 'Transition');
    unmount();

    start = clock();
    const button = document.querySelector('#glass');
    const glass = globalThis.mountGlass(button);
    await until(() => button.style.getPropertyValue('--glass-lens'), 'Glasswork');
    timingsMs.glassReady = clock() - start;
    const filter = getComputedStyle(button).backdropFilter;
    check('glassRefraction', glass.supported && glass.bezel > 0 && filter.includes('blur(') &&
      filter.includes('url(') && document.querySelector('feDisplacementMap'));
    glass.destroy();
    check('glassCleanup', !document.querySelector('feDisplacementMap') && !button.style.getPropertyValue('--glass-lens'));

    start = clock();
    const container = document.querySelector('#shadcn');
    globalThis.mountShadcn(true, false);
    const main = container.querySelector('main');
    const width = () => main.getBoundingClientRect().width;
    const bounded = width();
    const style = getComputedStyle(main);
    timingsMs.shadcnMount = clock() - start;
    check('shadcnLayout', main.dataset.layout === 'fixed' && style.display === 'flex' &&
      parseFloat(style.paddingLeft) > 0 && bounded > 0 && bounded < container.clientWidth);
    container.style.width = '640px';
    await frame();
    check('shadcnResize', width() < bounded && Math.abs(width() - container.clientWidth) <= 1);
    container.style.width = '1440px';
    globalThis.mountShadcn(false, true);
    await frame();
    check('shadcnFluid', main.dataset.layout === 'auto' && width() > bounded && Math.abs(width() - container.clientWidth) <= 1);
    await frames;
    report.ok = true;
  } catch (error) {
    report.error = String(error);
  } finally {
    globalThis.probeReport = report;
  }
})();
