import { useState } from 'react'
import { Badge } from '../vendor/shadcn-admin/src/components/ui/badge'
import { Button } from '../vendor/shadcn-admin/src/components/ui/button'
import { Input } from '../vendor/shadcn-admin/src/components/ui/input'
import { copyFor } from './copy'
import { useHost } from './shell'

type StorageResult = {
  enabled?: boolean
  storage?: string
  pending?: boolean
  deleted?: boolean
  code?: string
  id?: string
  capture?: Rung
  logs?: Rung
  deletable?: unknown[]
  held?: unknown[]
}

type Rung = {
  usage?: number
  first?: number
  repeat?: number
  declines?: number
  armed?: number
  pending?: boolean
  crossed?: number
  next?: number
}

async function storageCommand(body: Record<string, unknown>): Promise<StorageResult> {
  const response = await fetch('/command', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body),
  })
  const parsed = await response.json().catch(() => ({}))
  return parsed
}

export function StoragePage() {
  const state = useHost()
  const copy = copyFor(state.language)
  const plugin = state.plugins.find((item) => item.id === 'strata.storage')
  const [captureFirst, setCaptureFirst] = useState('1000000000')
  const [captureRepeat, setCaptureRepeat] = useState('1000000000')
  const [logFirst, setLogFirst] = useState('2000000000')
  const [logRepeat, setLogRepeat] = useState('500000000')
  const [selection, setSelection] = useState('')
  const [result, setResult] = useState<StorageResult | null>(null)

  async function refresh() {
    setResult(await storageCommand({ command: 'storage-status' }))
  }

  async function save() {
    setResult(await storageCommand({
      command: 'storage-configure',
      capture_first: Number(captureFirst),
      capture_repeat: Number(captureRepeat),
      log_first: Number(logFirst),
      log_repeat: Number(logRepeat),
    }))
  }

  async function decline(category: 'capture' | 'logs') {
    setResult(await storageCommand({ command: 'storage-decline', category }))
  }

  async function propose() {
    const ids = selection.split(',').map((item) => item.trim()).filter(Boolean)
    setResult(await storageCommand({
      command: 'storage-propose',
      payload: JSON.stringify({ op: 'selection', ids }),
    }))
  }

  async function confirm() {
    if (!result?.id) return
    setResult(await storageCommand({
      command: 'storage-confirm',
      proposal: result.id,
      confirm: true,
    }))
  }

  const running = plugin?.enabled === true && (plugin.availability === 'running' || plugin.availability === 'paused')
  const capture = result?.capture
  const logs = result?.logs
  return (
    <section className='flex flex-col gap-3'>
      <h1 className='text-2xl font-semibold'>{copy.storageTitle}</h1>
      <p>{copy.storageBody}</p>
      <div className='flex flex-wrap items-center gap-2'>
        <Badge variant='outline'>{result?.storage ?? copy.storageIdle}</Badge>
        <Badge variant={result?.pending ? 'default' : 'secondary'}>
          {result?.pending ? copy.storagePending : copy.storageQuiet}
        </Badge>
        <Badge variant={running ? 'default' : 'secondary'}>
          {running ? plugin?.availability ?? 'ready' : copy.storagePlugin}
        </Badge>
      </div>
      <p>
        {copy.storageUsage} {capture?.usage ?? 0} / {logs?.usage ?? 0}
      </p>
      <p>
        {copy.storageArmed} {capture?.armed ?? 0} / {logs?.armed ?? 0}
      </p>
      <p>
        {copy.storageCrossed} {capture?.crossed ?? 0} / {logs?.crossed ?? 0}
      </p>
      <p>
        {copy.storageNext} {capture?.next ?? 0} / {logs?.next ?? 0}
      </p>
      <p>
        {copy.storageProposal} {result?.id ?? ''} {copy.storageDeletable} {result?.deletable?.length ?? 0} {copy.storageHeld} {result?.held?.length ?? 0}
      </p>
      <label>
        {copy.storageCaptureFirst}
        <Input value={captureFirst} onChange={(event) => setCaptureFirst(event.target.value)} />
      </label>
      <label>
        {copy.storageCaptureRepeat}
        <Input value={captureRepeat} onChange={(event) => setCaptureRepeat(event.target.value)} />
      </label>
      <label>
        {copy.storageLogFirst}
        <Input value={logFirst} onChange={(event) => setLogFirst(event.target.value)} />
      </label>
      <label>
        {copy.storageLogRepeat}
        <Input value={logRepeat} onChange={(event) => setLogRepeat(event.target.value)} />
      </label>
      <div className='flex flex-wrap gap-2'>
        <Button type='button' onClick={save}>{copy.storageSave}</Button>
        <Button type='button' variant='outline' onClick={() => decline('capture')}>{copy.storageDeclineCapture}</Button>
        <Button type='button' variant='outline' onClick={() => decline('logs')}>{copy.storageDeclineLogs}</Button>
        <Button type='button' variant='outline' onClick={refresh}>{copy.storageRefresh}</Button>
      </div>
      <label>
        {copy.storageSelection}
        <Input value={selection} onChange={(event) => setSelection(event.target.value)} />
      </label>
      <div className='flex flex-wrap gap-2'>
        <Button type='button' variant='outline' onClick={propose}>{copy.storagePropose}</Button>
        <Button type='button' onClick={confirm}>{copy.storageConfirm}</Button>
      </div>
    </section>
  )
}
