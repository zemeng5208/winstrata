import { useState } from 'react'
import { Badge } from '../vendor/shadcn-admin/src/components/ui/badge'
import { Button } from '../vendor/shadcn-admin/src/components/ui/button'
import { Input } from '../vendor/shadcn-admin/src/components/ui/input'
import { copyFor } from './copy'
import { useHost } from './shell'

type Workbench = {
  enabled?: boolean
  presets?: number
  ok?: boolean
  code?: string
  id?: string
  pending?: boolean
  tokens?: unknown
  items?: unknown
}

const languages = ['python', 'javascript', 'typescript'] as const

function encode(text: string) {
  const bytes = new TextEncoder().encode(text)
  let binary = ''
  for (const byte of bytes) binary += String.fromCharCode(byte)
  return btoa(binary)
}

async function workbench(payload: string): Promise<Workbench> {
  const response = await fetch('/command', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ command: 'script-workbench', payload }),
  })
  const body = await response.json().catch(() => ({}))
  return body
}

async function sendSource(op: string, fields: Record<string, unknown>, source: string) {
  const data = encode(source)
  const size = 480
  if (data.length <= size) {
    return workbench(JSON.stringify({ op, ...fields, data }))
  }
  let part = 0
  let last: Workbench = {}
  for (let index = 0; index < data.length; index += size) {
    const end = index + size >= data.length
    last = await workbench(JSON.stringify({
      op,
      ...fields,
      part,
      end,
      data: data.slice(index, index + size),
    }))
    if (last.enabled === false || last.pending === false && last.ok === false) return last
    part += 1
  }
  return last
}

export function ScriptPage() {
  const state = useHost()
  const copy = copyFor(state.language)
  const plugin = state.plugins.find((item) => item.id === 'strata.script')
  const [language, setLanguage] = useState<(typeof languages)[number]>('python')
  const [name, setName] = useState('draft')
  const [prefix, setPrefix] = useState('')
  const [source, setSource] = useState('')
  const [scriptId, setScriptId] = useState('')
  const [result, setResult] = useState<Workbench | null>(null)

  async function check() {
    setResult(await workbench(JSON.stringify({ op: 'empty' })))
  }

  async function save() {
    const fields = scriptId
      ? { id: scriptId }
      : { language, name }
    const next = await sendSource(scriptId ? 'edit' : 'create', fields, source)
    if (typeof next.id === 'string') setScriptId(next.id)
    setResult(next)
  }

  async function ask(op: 'diagnose' | 'highlight' | 'complete') {
    if (!scriptId) {
      setResult(await workbench(JSON.stringify({ op: 'empty' })))
      return
    }
    const payload = op === 'complete'
      ? { op, id: scriptId, prefix }
      : { op, id: scriptId }
    setResult(await workbench(JSON.stringify(payload)))
  }

  const presets = result?.presets ?? 0
  const running = result?.enabled === true
  return (
    <section className='flex flex-col gap-3'>
      <h1 className='text-2xl font-semibold'>{copy.scriptsTitle}</h1>
      <p>{copy.scriptsBody}</p>
      <div className='flex flex-wrap items-center gap-2'>
        <Badge variant='outline'>{copy.scriptsPresets} {presets}</Badge>
        <Badge variant={running ? 'default' : 'secondary'}>
          {running ? plugin?.availability ?? 'ready' : copy.scriptsIdle}
        </Badge>
      </div>
      <div className='flex flex-wrap gap-2'>
        {languages.map((item) => (
          <Button
            key={item}
            type='button'
            variant={language === item ? 'default' : 'outline'}
            onClick={() => setLanguage(item)}
          >
            {item}
          </Button>
        ))}
      </div>
      <label className='flex flex-col gap-1 text-sm'>
        {copy.scriptsName}
        <Input value={name} onChange={(event) => setName(event.target.value)} />
      </label>
      <label className='flex flex-col gap-1 text-sm'>
        {copy.scriptsSource}
        <textarea
          value={source}
          onChange={(event) => setSource(event.target.value)}
          className='flex min-h-40 w-full rounded-md border border-input bg-transparent px-3 py-2 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50'
        />
      </label>
      <label className='flex flex-col gap-1 text-sm'>
        {copy.scriptsPrefix}
        <Input value={prefix} onChange={(event) => setPrefix(event.target.value)} />
      </label>
      <div className='flex flex-wrap gap-2'>
        <Button type='button' variant='outline' onClick={check}>{copy.scriptsCheck}</Button>
        <Button type='button' onClick={save}>{copy.scriptsSave}</Button>
        <Button type='button' variant='outline' onClick={() => ask('diagnose')}>{copy.scriptsDiagnose}</Button>
        <Button type='button' variant='outline' onClick={() => ask('highlight')}>{copy.scriptsHighlight}</Button>
        <Button type='button' variant='outline' onClick={() => ask('complete')}>{copy.scriptsComplete}</Button>
      </div>
      {result ? <pre className='overflow-auto text-xs'>{JSON.stringify(result)}</pre> : null}
    </section>
  )
}
