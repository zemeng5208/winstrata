// 行为块摘自 gallery/originals/index.html 的 standalone:ink-switch-script，字节与该段一致。
export function mountInk(scope) {
  const reducedMotion = matchMedia('(prefers-reduced-motion: reduce)');
  const motionEnabled = () => !reducedMotion.matches;
  const $ = (selector, base = scope) => base.querySelector(selector);
  const $$ = (selector, base = scope) => Array.from(base.querySelectorAll(selector));
    {
      const ink = $('.ink');
      const fill = $('.ink-fill', ink);
      const knob = $('.ink-knob', ink);
      const splash = $$('.ink-splash i', ink);
      let motions = [];

      function cancelMotions() {
        motions.forEach(motion => motion.cancel());
        motions = [];
      }
      reducedMotion.addEventListener('change', cancelMotions);

      ink.addEventListener('click', () => {
        const on = ink.getAttribute('aria-checked') !== 'true';
        ink.setAttribute('aria-checked', String(on));
        ink.dispatchEvent(new Event('change', { bubbles: true }));
        cancelMotions();
        if (!motionEnabled()) return;
        const easing = 'cubic-bezier(.2,.75,.25,1)';
        motions.push(knob.animate([
          { scale: '1 1' },
          { scale: '1.3 .84', offset: .32 },
          { scale: '.94 1.06', offset: .68 },
          { scale: '1 1' },
        ], { duration: 520, easing: 'cubic-bezier(.3,.7,.4,1)' }));
        if (on) {
          motions.push(fill.animate([
            { clipPath: 'circle(0px at 23px 50%)' },
            { clipPath: 'circle(14px at 23px 50%)', offset: .18 },
            { clipPath: 'circle(80px at 23px 50%)' },
          ], { duration: 640, easing }));
          splash.forEach((drop, index) => {
            const angle = (-60 + index * 60) * Math.PI / 180;
            const x = Math.cos(angle) * (22 + index * 4), y = Math.sin(angle) * 14;
            motions.push(drop.animate([
              { opacity: 1, transform: 'translate(0, 0) scale(1)' },
              { opacity: 1, transform: `translate(${x}px, ${y}px) scale(.9)`, offset: .55 },
              { opacity: 0, transform: `translate(${x * 1.25}px, ${y * 1.25}px) scale(0)` },
            ], { duration: 460, delay: 40 + index * 30, easing: 'cubic-bezier(.2,.8,.3,1)' }));
          });
        } else {
          motions.push(fill.animate([
            { clipPath: 'circle(80px at 61px 50%)' },
            { clipPath: 'circle(18px at 61px 50%)', offset: .7 },
            { clipPath: 'circle(0px at 61px 50%)' },
          ], { duration: 460, easing: 'cubic-bezier(.5,0,.3,1)' }));
        }
      });
    }
}
