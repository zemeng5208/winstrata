import { useState } from 'react'
import { Badge } from '../vendor/shadcn-admin/src/components/ui/badge'
import { Button } from '../vendor/shadcn-admin/src/components/ui/button'
import { Input } from '../vendor/shadcn-admin/src/components/ui/input'
import { copyFor } from './copy'
import { useHost } from './shell'

type TagResult = {
  enabled?: boolean
  ok?: boolean
  code?: string
  safety?: string
  actor?: string
  wrote?: boolean
}

const kinds = ['event', 'packet', 'file', 'process', 'software', 'registry', 'system'] as const
const bases = ['observed', 'inferred', 'unknown'] as const

async function tagApply(payload: string): Promise<TagResult> {
  const response = await fetch('/command', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ command: 'tag-apply', payload }),
  })
  const body = await response.json().catch(() => ({}))
  return body
}

export function TagPage() {
  const state = useHost()
  const copy = copyFor(state.language)
  const plugin = state.plugins.find((item) => item.id === 'strata.tag')
  const [kind, setKind] = useState<(typeof kinds)[number]>('event')
  const [subject, setSubject] = useState('')
  const [reason, setReason] = useState('')
  const [source, setSource] = useState('')
  const [confidence, setConfidence] = useState('')
  const [evidence, setEvidence] = useState('')
  const [attribution, setAttribution] = useState<(typeof bases)[number]>('observed')
  const [packet, setPacket] = useState('')
  const [result, setResult] = useState<TagResult | null>(null)

  async function apply() {
    const body: Record<string, unknown> = {
      kind,
      subject,
      reason,
      source,
      confidence,
      evidence,
      attribution,
    }
    if (kind === 'packet' && packet.trim() !== '') body.packet = Number(packet)
    setResult(await tagApply(JSON.stringify(body)))
  }

  const running = plugin?.enabled === true && (plugin.availability === 'running' || plugin.availability === 'paused')
  return (
    <section className='flex flex-col gap-3'>
      <h1 className='text-2xl font-semibold'>{copy.tagsTitle}</h1>
      <p>{copy.tagsBody}</p>
      <div className='flex flex-wrap items-center gap-2'>
        <Badge variant='outline'>{copy.tagsSafety}</Badge>
        <Badge variant={running ? 'default' : 'secondary'}>
          {running ? plugin?.availability ?? 'ready' : copy.tagsIdle}
        </Badge>
      </div>
      <div className='flex flex-wrap gap-2'>
        {kinds.map((item) => (
          <Button
            key={item}
            type='button'
            variant={kind === item ? 'default' : 'outline'}
            onClick={() => setKind(item)}
          >
            {item}
          </Button>
        ))}
      </div>
      <label className='flex flex-col gap-1 text-sm'>
        {copy.tagsSubject}
        <Input value={subject} onChange={(event) => setSubject(event.target.value)} />
      </label>
      <label className='flex flex-col gap-1 text-sm'>
        {copy.tagsReason}
        <Input value={reason} onChange={(event) => setReason(event.target.value)} />
      </label>
      <label className='flex flex-col gap-1 text-sm'>
        {copy.tagsSource}
        <Input value={source} onChange={(event) => setSource(event.target.value)} />
      </label>
      <label className='flex flex-col gap-1 text-sm'>
        {copy.tagsConfidence}
        <Input value={confidence} onChange={(event) => setConfidence(event.target.value)} />
      </label>
      <label className='flex flex-col gap-1 text-sm'>
        {copy.tagsEvidence}
        <Input value={evidence} onChange={(event) => setEvidence(event.target.value)} />
      </label>
      <label className='flex flex-col gap-1 text-sm'>
        {copy.tagsPacket}
        <Input value={packet} onChange={(event) => setPacket(event.target.value)} />
      </label>
      <div className='flex flex-wrap gap-2'>
        {bases.map((item) => (
          <Button
            key={item}
            type='button'
            variant={attribution === item ? 'default' : 'outline'}
            onClick={() => setAttribution(item)}
          >
            {item}
          </Button>
        ))}
      </div>
      <div>
        <Button type='button' onClick={apply}>{copy.tagsApply}</Button>
      </div>
      {result ? <pre className='overflow-auto text-xs'>{JSON.stringify(result)}</pre> : null}
    </section>
  )
}
