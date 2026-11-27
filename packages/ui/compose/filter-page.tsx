import { useState } from 'react'
import { Badge } from '../vendor/shadcn-admin/src/components/ui/badge'
import { Button } from '../vendor/shadcn-admin/src/components/ui/button'
import { Input } from '../vendor/shadcn-admin/src/components/ui/input'
import { copyFor } from './copy'
import { useHost } from './shell'

type FilterResult = {
  enabled?: boolean
  ok?: boolean
  phase?: string
  kept?: number
  avoided?: number
  overhead?: number
  suppressed?: number
  code?: string
}

const fields = ['path', 'event', 'process', 'app', 'protocol', 'ip', 'port', 'iface'] as const

async function apply(payload: string): Promise<FilterResult> {
  const response = await fetch('/command', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ command: 'filter-apply', payload }),
  })
  const body = await response.json().catch(() => ({}))
  return body
}

export function FilterPage() {
  const state = useHost()
  const copy = copyFor(state.language)
  const plugin = state.plugins.find((item) => item.id === 'strata.filter')
  const [field, setField] = useState<(typeof fields)[number]>('path')
  const [mode, setMode] = useState<'include' | 'exclude'>('exclude')
  const [value, setValue] = useState('')
  const [result, setResult] = useState<FilterResult | null>(null)

  async function save() {
    setResult(await apply(JSON.stringify({ op: 'rule', field, mode, value })))
  }

  async function show() {
    setResult(await apply(JSON.stringify({ op: 'show' })))
  }

  const running = result?.enabled === true
  return (
    <section className='flex flex-col gap-3'>
      <h1 className='text-2xl font-semibold'>{copy.filtersTitle}</h1>
      <p>{copy.filtersBody}</p>
      <div className='flex flex-wrap items-center gap-2'>
        <Badge variant='outline'>{copy.filtersShow} {result?.kept ?? 0}</Badge>
        <Badge variant='outline'>{result?.avoided ?? 0}</Badge>
        <Badge variant='outline'>{result?.overhead ?? 0}</Badge>
        <Badge variant={running ? 'default' : 'secondary'}>
          {running ? plugin?.availability ?? 'ready' : copy.filtersIdle}
        </Badge>
      </div>
      <div className='flex flex-wrap gap-2'>
        {fields.map((item) => (
          <Button
            key={item}
            type='button'
            variant={field === item ? 'default' : 'outline'}
            onClick={() => setField(item)}
          >
            {item}
          </Button>
        ))}
      </div>
      <div className='flex flex-wrap gap-2'>
        <Button type='button' variant={mode === 'include' ? 'default' : 'outline'} onClick={() => setMode('include')}>
          {copy.filtersInclude}
        </Button>
        <Button type='button' variant={mode === 'exclude' ? 'default' : 'outline'} onClick={() => setMode('exclude')}>
          {copy.filtersExclude}
        </Button>
      </div>
      <label className='flex flex-col gap-1 text-sm'>
        {copy.filtersField} {field}
        <span>{copy.filtersValue}</span>
        <Input value={value} onChange={(event) => setValue(event.target.value)} />
      </label>
      <div className='flex flex-wrap gap-2'>
        <Button type='button' onClick={save}>{copy.filtersSave}</Button>
        <Button type='button' variant='outline' onClick={show}>{copy.filtersShow}</Button>
      </div>
      {result ? <pre className='overflow-auto text-xs'>{JSON.stringify(result)}</pre> : null}
    </section>
  )
}
