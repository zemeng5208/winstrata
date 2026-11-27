import { createContext, useContext, useEffect, useState } from 'react'
import { Outlet, Link } from '@tanstack/react-router'
import { Activity, Blocks, LayoutDashboard, Settings, SquareTerminal, Tags } from 'lucide-react'
import { FadeIn } from '../vendor/amicro/fade-in'
import { liquidGlass } from '../vendor/glasswork/lens.js'
import { AppTitle } from '../vendor/shadcn-admin/src/components/layout/app-title'
import { AuthenticatedLayout } from '../vendor/shadcn-admin/src/components/layout/authenticated-layout'
import { Header } from '../vendor/shadcn-admin/src/components/layout/header'
import { Main } from '../vendor/shadcn-admin/src/components/layout/main'
import { NavGroup } from '../vendor/shadcn-admin/src/components/layout/nav-group'
import { Button } from '../vendor/shadcn-admin/src/components/ui/button'
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarRail,
} from '../vendor/shadcn-admin/src/components/ui/sidebar'
import { useLayout } from '../vendor/shadcn-admin/src/context/layout-provider'
import { copyFor, type Copy, type Language } from './copy'
import { InkSwitch, SignalReturn } from './moments'

export type ThemeName = 'white-acrylic' | 'black-acrylic' | 'liquid-glass'
export type CloseName = 'ask' | 'tray' | 'exit'
export type PageName = 'overview' | 'plugins' | 'service' | 'settings' | 'scripts' | 'tags'

export type ScopeState = {
  roots: string[]
  fileTypes: string[]
  processes: string[]
  software: string[]
  registry: string[]
  interfaces: string[]
  watchProcesses: boolean
}

export type OnboardingState = {
  step: string
  complete: boolean
  skipped: string[]
  language: string | null
  optedIn: string[]
  rescue: boolean
  mcp: boolean
  encryption: boolean
  baseline: string | null
  baselineLabel: string | null
  retentionDays: number | null
  scripts: boolean
  automaticSnapshots: boolean
  scope: ScopeState
}

export type MarkerState = {
  sequence: number
  activity: string
  reason: string
  releaseDrivers: boolean
}

export type HostState = {
  theme: ThemeName | null
  language: Language | null
  close: CloseName | null
  closePending: boolean
  user: string
  storage: string
  run: string
  service: string
  plugins: { id: string; name: string; availability: string; enabled: boolean }[]
  privilege: string
  paused: boolean
  collectors: string
  drivers: string
  onboarding: OnboardingState
  markers: MarkerState[]
  markersNote: string | null
  settings: { startAtLogin: boolean; monitoredRoots: string[] }
}

const emptyScope: ScopeState = {
  roots: [],
  fileTypes: [],
  processes: [],
  software: [],
  registry: [],
  interfaces: [],
  watchProcesses: false,
}

const emptyOnboarding: OnboardingState = {
  step: 'language',
  complete: false,
  skipped: [],
  language: null,
  optedIn: [],
  rescue: false,
  mcp: false,
  encryption: false,
  baseline: null,
  baselineLabel: null,
  retentionDays: null,
  scripts: false,
  automaticSnapshots: false,
  scope: emptyScope,
}

const emptyState: HostState = {
  theme: null,
  language: null,
  close: null,
  closePending: false,
  user: '',
  storage: '',
  run: '',
  service: '',
  plugins: [],
  privilege: '',
  paused: false,
  collectors: '',
  drivers: '',
  onboarding: emptyOnboarding,
  markers: [],
  markersNote: null,
  settings: { startAtLogin: false, monitoredRoots: [] },
}

const HostContext = createContext<HostState>(emptyState)

export function useHost() {
  return useContext(HostContext)
}

async function command(body: Record<string, unknown>) {
  const response = await fetch('/command', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body),
  })
  const payload = await response.json().catch(() => ({}))
  if (!response.ok) {
    return typeof payload.error === 'string' ? payload.error : String(response.status)
  }
  return null
}

function lines(value: string) {
  return value.split(/\r?\n/).map((item) => item.trim()).filter(Boolean)
}

function useCommand() {
  const [notice, setNotice] = useState<string | null>(null)
  async function run(body: Record<string, unknown>) {
    setNotice(await command(body))
  }
  return { notice, run }
}

function glassProps(theme: ThemeName | null) {
  if (!theme) return { className: '', kind: undefined as string | undefined, style: undefined }
  const kind = theme === 'black-acrylic' ? 'dark' : 'light'
  return {
    className: 'glass',
    kind,
    // shadcn 的 --radius 使用 rem。Glasswork 这里要的是自己的无单位半径。
    // 内层 bg-sidebar 改成透明，磨砂才落在上游侧栏容器上。
    style: { '--radius': '22', '--sidebar': 'transparent' } as React.CSSProperties,
  }
}

function OfficialSidebar({ copy, user, theme }: { copy: Copy; user: string; theme: ThemeName | null }) {
  const { collapsible, variant } = useLayout()
  const glass = glassProps(theme)
  return (
    <Sidebar collapsible={collapsible} variant={variant} className={glass.className} data-kind={glass.kind} style={glass.style}>
      <SidebarHeader>
        <AppTitle title={copy.product} subtitle={copy.subtitle} />
      </SidebarHeader>
      <SidebarContent>
        <NavGroup
          title={copy.navigation}
          items={[
            { title: copy.overview, url: '/', icon: LayoutDashboard },
            { title: copy.plugins, url: '/plugins', icon: Blocks },
            { title: copy.scripts, url: '/scripts', icon: SquareTerminal },
            { title: copy.tags, url: '/tags', icon: Tags },
            { title: copy.service, url: '/service', icon: Activity },
            { title: copy.settings, url: '/settings', icon: Settings },
          ]}
        />
      </SidebarContent>
      <SidebarFooter>
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton size='lg'>
              <div className='grid flex-1 text-start text-sm leading-tight'>
                <span className='truncate font-semibold'>{user || copy.user}</span>
                <span className='truncate text-xs'>{copy.officialGui}</span>
              </div>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarFooter>
      <SidebarRail />
    </Sidebar>
  )
}

function ThemeButton({ copy, theme, active, label }: { copy: Copy; theme: ThemeName; active: boolean; label: string }) {
  const glass = theme === 'liquid-glass'
  return (
    <Button
      type='button'
      variant={active ? 'default' : 'outline'}
      className={glass ? 'glass' : undefined}
      data-kind={glass ? 'light' : undefined}
      data-lens={glass ? '' : undefined}
      onClick={() => command({ command: 'set-theme', theme })}
    >
      {label}
      <span className='sr-only'>{copy.theme}</span>
    </Button>
  )
}

export function Shell() {
  const [state, setState] = useState<HostState>(emptyState)
  const copy = copyFor(state.language)
  const glass = glassProps(state.theme)

  useEffect(() => {
    let stop = false
    const pull = () => {
      fetch('/state')
        .then((response) => response.json())
        .then((next: HostState) => {
          if (!stop) {
            setState({
              ...emptyState,
              ...next,
              onboarding: { ...emptyOnboarding, ...next.onboarding, scope: { ...emptyScope, ...next.onboarding?.scope } },
              settings: { ...emptyState.settings, ...next.settings },
              markers: next.markers ?? [],
              plugins: next.plugins ?? [],
            })
          }
        })
        .catch(() => undefined)
    }
    pull()
    const timer = window.setInterval(pull, 1000)
    return () => {
      stop = true
      window.clearInterval(timer)
    }
  }, [])

  useEffect(() => {
    const root = document.documentElement
    root.classList.remove('dark')
    if (state.theme === 'black-acrylic') root.classList.add('dark')
    if (state.theme) root.dataset.theme = state.theme
    else delete root.dataset.theme
    root.lang = state.language === 'zh-CN' ? 'zh-CN' : 'en'
    const lenses = Array.from(document.querySelectorAll<HTMLElement>('[data-lens]'))
    const mounted = state.theme === 'liquid-glass' ? lenses.map((node) => liquidGlass(node)) : []
    return () => {
      for (const item of mounted) item.destroy()
    }
  }, [state.theme, state.language])

  return (
    <>
      <HostContext.Provider value={state}>
      <div
        aria-hidden='true'
        style={{
          position: 'fixed',
          inset: 0,
          zIndex: -1,
          background:
            'linear-gradient(160deg, var(--chart-2), var(--background) 46%, var(--chart-4))',
        }}
      />
      <AuthenticatedLayout
        skipLabel={copy.skip}
        sidebar={<OfficialSidebar copy={copy} user={state.user} theme={state.theme} />}
      >
        <Header fixed className={glass.className} data-kind={glass.kind} style={glass.style}>
          <span className='text-sm font-medium'>{copy.product}</span>
          <div className='ms-auto flex items-center gap-2'>
            <Button type='button' variant='outline' asChild>
              <Link to='/settings'>{copy.settings}</Link>
            </Button>
          </div>
        </Header>
        <Main fixed id='content' className={glass.className} data-kind={glass.kind} style={glass.style}>
          <FadeIn>
            {state.closePending ? <p>{copy.closePending}</p> : null}
            <Outlet />
          </FadeIn>
        </Main>
      </AuthenticatedLayout>
      </HostContext.Provider>
    </>
  )
}

function Field({ label, value, onChange }: { label: string; value: string; onChange: (value: string) => void }) {
  return (
    <label className='flex flex-col gap-1 text-sm'>
      {label}
      <textarea className='border-input min-h-20 rounded-md border bg-transparent px-3 py-2' value={value} onChange={(event) => onChange(event.target.value)} />
    </label>
  )
}

function ScopeForm({ copy, scope, dark }: { copy: Copy; scope: ScopeState; dark: boolean }) {
  const { notice, run } = useCommand()
  const [roots, setRoots] = useState(scope.roots.join('\n'))
  const [types, setTypes] = useState(scope.fileTypes.join('\n'))
  const [processes, setProcesses] = useState(scope.processes.join('\n'))
  const [software, setSoftware] = useState(scope.software.join('\n'))
  const [registry, setRegistry] = useState(scope.registry.join('\n'))
  const [interfaces, setInterfaces] = useState(scope.interfaces.join('\n'))
  const [watch, setWatch] = useState(scope.watchProcesses)
  return (
    <div className='flex flex-col gap-3'>
      <h2 className='text-lg font-semibold'>{copy.scopeTitle}</h2>
      <p>{copy.scopeBody}</p>
      <Field label={copy.rootsLabel} value={roots} onChange={setRoots} />
      <Field label={copy.typesLabel} value={types} onChange={setTypes} />
      <Field label={copy.processesLabel} value={processes} onChange={setProcesses} />
      <Field label={copy.softwareLabel} value={software} onChange={setSoftware} />
      <Field label={copy.registryLabel} value={registry} onChange={setRegistry} />
      <Field label={copy.interfacesLabel} value={interfaces} onChange={setInterfaces} />
      <InkSwitch checked={watch} dark={dark} label={copy.watchProcesses} onChange={setWatch} />
      <Button
        type='button'
        onClick={() => run({
          command: 'set-scope',
          roots: lines(roots),
          fileTypes: lines(types),
          processes: lines(processes),
          software: lines(software),
          registry: lines(registry),
          interfaces: lines(interfaces),
          watchProcesses: watch || lines(processes).length > 0,
        })}
      >
        {copy.saveScope}
      </Button>
      {notice ? <p>{notice}</p> : null}
    </div>
  )
}

function OnboardingPanel({ state }: { state: HostState }) {
  const copy = copyFor(state.language)
  const { notice, run } = useCommand()
  const [root, setRoot] = useState('')
  const [days, setDays] = useState('')
  const [boot, setBoot] = useState(false)
  const [password, setPassword] = useState('')
  const [lock, setLock] = useState('0')
  const [mcp, setMcp] = useState(false)
  const [chosen, setChosen] = useState<string[]>([])
  const step = state.onboarding.step
  const dark = state.theme === 'black-acrylic'
  return (
    <section className='flex flex-col gap-3'>
      <h2 className='text-lg font-semibold'>{copy.onboardingTitle}</h2>
      <p>{copy.onboardingBody}</p>
      <p>{copy.step}: {step}</p>
      {step === 'language' ? (
        <div className='flex flex-col gap-2'>
          <label className='flex flex-col gap-1 text-sm'>
            {copy.locationLabel}
            <input className='border-input rounded-md border bg-transparent px-3 py-2' value={root} onChange={(event) => setRoot(event.target.value)} />
          </label>
          <div className='flex flex-wrap gap-2'>
            <Button type='button' onClick={() => run({ command: 'choose-location', root, language: state.language ?? 'en' })}>{copy.useLocation}</Button>
            <Button type='button' variant='outline' onClick={() => run({ command: 'choose-location', root, skipLanguage: true })}>{copy.skipLanguage}</Button>
          </div>
        </div>
      ) : null}
      {step === 'plugins' ? (
        <div className='flex flex-col gap-2'>
          <p>{copy.pluginsBody}</p>
          <ul className='flex flex-col gap-2'>
            {state.plugins.filter((plugin) => plugin.id !== 'strata.rescue').map((plugin) => (
              <li key={plugin.id}>
                <label className='flex items-center gap-2 text-sm'>
                  <input
                    type='checkbox'
                    checked={chosen.includes(plugin.id)}
                    onChange={(event) => {
                      setChosen((current) => event.target.checked
                        ? [...current, plugin.id]
                        : current.filter((id) => id !== plugin.id))
                    }}
                  />
                  {plugin.name} ({plugin.id}) {plugin.availability} {String(plugin.enabled)}
                </label>
              </li>
            ))}
          </ul>
          <p>{copy.rescueBody}</p>
          <InkSwitch checked={state.onboarding.rescue} dark={dark} label={copy.rescue} onChange={(enabled) => run({ command: 'set-rescue', enabled })} />
          <div className='flex flex-wrap gap-2'>
            <Button type='button' onClick={() => run({ command: 'opt-in', ids: chosen.filter((id) => id !== 'strata.rescue') })}>{copy.plugins}</Button>
            <Button type='button' variant='outline' onClick={() => run({ command: 'skip-onboarding', step: 'plugins' })}>{copy.skipStep}</Button>
          </div>
        </div>
      ) : null}
      {step === 'scope' ? <ScopeForm copy={copy} scope={state.onboarding.scope} dark={dark} /> : null}
      {step === 'retention' ? (
        <div className='flex flex-col gap-2'>
          <p>{copy.retentionBody}</p>
          <label className='flex flex-col gap-1 text-sm'>
            {copy.retentionLabel}
            <input className='border-input rounded-md border bg-transparent px-3 py-2' value={days} onChange={(event) => setDays(event.target.value)} />
          </label>
          <div className='flex flex-wrap gap-2'>
            <Button type='button' onClick={() => run({ command: 'confirm-retention', days: days.trim() === '' ? null : Number(days) })}>{copy.retentionLabel}</Button>
            <Button type='button' variant='outline' onClick={() => run({ command: 'skip-onboarding', step: 'retention' })}>{copy.skipStep}</Button>
          </div>
        </div>
      ) : null}
      {step === 'boot' ? (
        <div className='flex flex-col gap-2'>
          <InkSwitch checked={boot} dark={dark} label={copy.bootLabel} onChange={setBoot} />
          <div className='flex flex-wrap gap-2'>
            <Button type='button' onClick={() => run({ command: 'set-boot', startAtLogin: boot })}>{copy.bootLabel}</Button>
            <Button type='button' variant='outline' onClick={() => run({ command: 'skip-onboarding', step: 'boot' })}>{copy.skipStep}</Button>
          </div>
        </div>
      ) : null}
      {step === 'encryption' ? (
        <div className='flex flex-col gap-2'>
          <p>{copy.encryptionBody}</p>
          <label className='flex flex-col gap-1 text-sm'>
            {copy.encryptionPassword}
            <input className='border-input rounded-md border bg-transparent px-3 py-2' type='password' value={password} onChange={(event) => setPassword(event.target.value)} />
          </label>
          <label className='flex flex-col gap-1 text-sm'>
            {copy.autoLock}
            <input className='border-input rounded-md border bg-transparent px-3 py-2' value={lock} onChange={(event) => setLock(event.target.value)} />
          </label>
          <div className='flex flex-wrap gap-2'>
            <Button type='button' onClick={() => run({ command: 'enable-encryption', password, autoLockSeconds: Number(lock) })}>{copy.encryptionLabel}</Button>
            <Button type='button' variant='outline' onClick={() => run({ command: 'skip-onboarding', step: 'encryption' })}>{copy.skipStep}</Button>
          </div>
        </div>
      ) : null}
      {step === 'mcp' ? (
        <div className='flex flex-col gap-2'>
          <InkSwitch checked={mcp} dark={dark} label={copy.mcpLabel} onChange={setMcp} />
          <div className='flex flex-wrap gap-2'>
            <Button type='button' onClick={() => run({ command: 'set-mcp', enabled: mcp })}>{copy.mcpLabel}</Button>
            <Button type='button' variant='outline' onClick={() => run({ command: 'skip-onboarding', step: 'mcp' })}>{copy.skipStep}</Button>
          </div>
        </div>
      ) : null}
      {step === 'baseline' ? (
        <div className='flex flex-col gap-2'>
          <h2 className='text-lg font-semibold'>{copy.baselineTitle}</h2>
          <p>{copy.baselineBody}</p>
          <div className='flex flex-wrap gap-2'>
            <Button type='button' onClick={() => run({ command: 'build-baseline' })}>{copy.buildBaseline}</Button>
            <Button type='button' variant='outline' onClick={() => run({ command: 'skip-onboarding', step: 'baseline' })}>{copy.skipStep}</Button>
          </div>
        </div>
      ) : null}
      {notice ? <p>{notice}</p> : null}
    </section>
  )
}

function ModePanel({ state }: { state: HostState }) {
  const copy = copyFor(state.language)
  const { notice, run } = useCommand()
  return (
    <section className='flex flex-col gap-3'>
      <h2 className='text-lg font-semibold'>{copy.modeTitle}</h2>
      <p>{copy.modeBody}</p>
      <p>{copy.run}: {state.run}</p>
      <p>{copy.paused}: {state.paused ? copy.paused : copy.run}</p>
      <p>{copy.collectors}: {state.collectors || copy.pluginsEmpty}</p>
      <p>{copy.drivers}: {state.drivers || copy.serviceDown}</p>
      <p>{copy.noLossButton}</p>
      <div className='flex flex-wrap gap-2'>
        <Button type='button' onClick={() => run({ command: 'start-live' })}>{copy.live}</Button>
        <Button type='button' variant='outline' onClick={() => run({ command: 'pause' })}>{copy.pause}</Button>
        <Button type='button' variant='outline' onClick={() => run({ command: 'resume' })}>{copy.resume}</Button>
        <Button type='button' variant='outline' onClick={() => run({ command: 'enter-history' })}>{copy.history}</Button>
        <Button type='button' variant='outline' onClick={() => run({ command: 'stop' })}>{copy.stopCollection}</Button>
      </div>
      <h3 className='font-medium'>{copy.markers}</h3>
      {state.markers.length === 0 ? <p>{copy.emptyMarkers}</p> : null}
      {state.markersNote ? <p>{state.markersNote}</p> : null}
      <ul>
        {state.markers.map((marker) => (
          <li key={marker.sequence}>{marker.sequence} {marker.activity} {marker.reason} {String(marker.releaseDrivers)}</li>
        ))}
      </ul>
      {notice ? <p>{notice}</p> : null}
    </section>
  )
}

export function OverviewPage() {
  const state = useHost()
  const copy = copyFor(state.language)
  const located = state.settings.monitoredRoots.length > 0 || state.onboarding.complete
  return (
    <section className='flex flex-col gap-4'>
      <h1 className='text-2xl font-semibold'>{copy.overview}</h1>
      <p>{copy.dashboardEmpty}</p>
      <p>{copy.widgetsLater}</p>
      <p>{copy.user}: {state.user}</p>
      <p>{copy.storage}: {state.storage}</p>
      <p>{copy.run}: {state.run}</p>
      <p>{copy.recovery}</p>
      {state.onboarding.complete ? null : <OnboardingPanel state={state} />}
      {located ? <ModePanel state={state} /> : null}
    </section>
  )
}

export function PluginsPage() {
  const state = useHost()
  const copy = copyFor(state.language)
  const { notice, run } = useCommand()
  const dark = state.theme === 'black-acrylic'
  const batch = state.plugins.filter((plugin) => plugin.id !== 'strata.rescue')
  return (
    <section className='flex flex-col gap-3'>
      <h1 className='text-2xl font-semibold'>{copy.plugins}</h1>
      <p>{copy.pluginsBody}</p>
      {state.plugins.length === 0 ? <p>{copy.pluginsEmpty}</p> : null}
      <ul className='flex flex-col gap-2'>
        {batch.map((plugin) => (
          <li key={plugin.id}>
            <label className='flex items-center gap-2 text-sm'>
              <input
                type='checkbox'
                checked={state.onboarding.optedIn.includes(plugin.id)}
                onChange={(event) => {
                  const ids = event.target.checked
                    ? [...state.onboarding.optedIn.filter((id) => id !== 'strata.rescue'), plugin.id]
                    : state.onboarding.optedIn.filter((id) => id !== plugin.id && id !== 'strata.rescue')
                  run({ command: 'opt-in', ids })
                }}
              />
              {plugin.name} ({plugin.id}) {plugin.availability} {String(plugin.enabled)}
            </label>
          </li>
        ))}
      </ul>
      <p>{copy.rescueBody}</p>
      <InkSwitch
        checked={state.onboarding.rescue}
        dark={dark}
        label={copy.rescue}
        onChange={(enabled) => run({ command: 'set-rescue', enabled })}
      />
      {notice ? <p>{notice}</p> : null}
    </section>
  )
}

export function ServicePage() {
  const state = useHost()
  const copy = copyFor(state.language)
  return (
    <section>
      <h1 className='text-2xl font-semibold'>{copy.service}</h1>
      {state.service.startsWith('connected') ? null : (
        <SignalReturn dark={state.theme === 'black-acrylic'} label={copy.service} />
      )}
      <p>{state.service || copy.serviceDown}</p>
      <p>{state.privilege}</p>
    </section>
  )
}

export function SettingsPage() {
  const state = useHost()
  const copy = copyFor(state.language)
  return (
    <section className='flex flex-col gap-4'>
      <h1 className='text-2xl font-semibold'>{copy.settings}</h1>
      <p>{state.theme ? null : copy.themeUnset}</p>
      <div className='flex flex-wrap gap-2'>
        <ThemeButton copy={copy} theme='white-acrylic' active={state.theme === 'white-acrylic'} label={copy.whiteAcrylic} />
        <ThemeButton copy={copy} theme='black-acrylic' active={state.theme === 'black-acrylic'} label={copy.blackAcrylic} />
        <ThemeButton copy={copy} theme='liquid-glass' active={state.theme === 'liquid-glass'} label={copy.liquidGlass} />
      </div>
      <div className='flex items-center gap-3'>
        <InkSwitch
          checked={state.language === 'zh-CN'}
          dark={state.theme === 'black-acrylic'}
          label={`${copy.language}: ${state.language === 'zh-CN' ? copy.chinese : copy.english}`}
          onChange={(on) => command({ command: 'set-language', language: on ? 'zh-CN' : 'en' })}
        />
        <span>{state.language === 'zh-CN' ? copy.chinese : copy.english}</span>
      </div>
      <div className='flex flex-wrap gap-2'>
        <Button type='button' variant={state.close === 'ask' || state.close === null ? 'default' : 'outline'} onClick={() => command({ command: 'set-close', close: 'ask' })}>
          {copy.closeAsk}
        </Button>
        <Button type='button' variant={state.close === 'tray' ? 'default' : 'outline'} onClick={() => command({ command: 'set-close', close: 'tray' })}>
          {copy.closeTray}
        </Button>
        <Button type='button' variant={state.close === 'exit' ? 'default' : 'outline'} onClick={() => command({ command: 'set-close', close: 'exit' })}>
          {copy.closeExit}
        </Button>
      </div>
      {state.settings.monitoredRoots.length > 0 ? (
        <ScopeForm copy={copy} scope={state.onboarding.scope} dark={state.theme === 'black-acrylic'} />
      ) : null}
    </section>
  )
}
