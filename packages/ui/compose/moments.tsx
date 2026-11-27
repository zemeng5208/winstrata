import { useEffect, useRef } from 'react'
import coreHtml from '../vendor/ui-moments/gallery/originals/index.html'
import { originalLoaders, originalMarkup } from '../vendor/ui-moments/gallery/loaders/originals.mjs'
import { mountInk } from './ink-switch.js'

function between(source: string, start: string, end: string) {
  const from = source.indexOf(start)
  const to = source.indexOf(end, from + start.length)
  if (from < 0 || to < 0) throw new Error(start)
  return source.slice(from + start.length, to)
}

function inkCss() {
  // 阴影根里 :host 承接上游写在 :root 上的令牌，避免改掉 shadcn 外壳的 data-theme。
  return [
    between(coreHtml, '/* standalone:theme */', '/* /standalone:theme */'),
    between(coreHtml, '/* standalone:ink-switch-style */', '/* /standalone:ink-switch-style */'),
    between(coreHtml, '/* standalone:reduced-motion */', '/* /standalone:reduced-motion */'),
    'button { cursor: pointer; -webkit-tap-highlight-color: transparent; }',
    ':focus-visible { outline: 2px solid var(--accent); outline-offset: 4px; }',
  ].join('\n').replaceAll(':root', ':host')
}

function inkButton() {
  const section = between(coreHtml, '<!-- standalone:ink-switch -->', '<!-- /standalone:ink-switch -->')
  const start = section.indexOf('<button class="ink"')
  const end = section.indexOf('</button>', start)
  return section.slice(start, end + '</button>'.length)
}

export function InkSwitch({
  checked,
  label,
  dark,
  onChange,
}: {
  checked: boolean
  label: string
  dark: boolean
  onChange: (checked: boolean) => void
}) {
  const host = useRef<HTMLDivElement>(null)
  const onChangeRef = useRef(onChange)
  onChangeRef.current = onChange

  useEffect(() => {
    const node = host.current
    if (!node) return
    const shadow = node.shadowRoot ?? node.attachShadow({ mode: 'open' })
    if (shadow.childNodes.length > 0) return
    shadow.innerHTML = `<style>${inkCss()}</style>${inkButton()}`
    const ink = shadow.querySelector<HTMLButtonElement>('.ink')
    ink?.addEventListener('change', () => {
      onChangeRef.current(ink.getAttribute('aria-checked') === 'true')
    })
    mountInk(shadow)
  }, [])

  useEffect(() => {
    const node = host.current
    const ink = node?.shadowRoot?.querySelector('.ink')
    if (!node || !ink) return
    node.dataset.theme = dark ? 'dark' : 'light'
    ink.setAttribute('aria-checked', String(checked))
    ink.setAttribute('aria-label', label)
  }, [checked, dark, label])

  return <div ref={host} />
}

export function SignalReturn({ dark, label }: { dark: boolean; label: string }) {
  const item = originalLoaders.find((entry: { key: string }) => entry.key === 'signal-return')
  if (!item) return null
  const markup = originalMarkup(item, {
    size: item.defaults.size,
    color: dark ? item.darkColor : item.defaults.color,
    speed: item.defaults.speed,
  })
  return <div role='status' aria-label={label} dangerouslySetInnerHTML={{ __html: markup }} />
}
