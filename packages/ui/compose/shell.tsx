import { createContext, useContext, useEffect, useState } from 'react'
import { Outlet, Link } from '@tanstack/react-router'
import { Activity, Blocks, LayoutDashboard, Settings } from 'lucide-react'
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
export type PageName = 'overview' | 'plugins' | 'service' | 'settings'

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
}

const HostContext = createContext<HostState>(emptyState)

function useHost() {
  return useContext(HostContext)
}

async function command(body: Record<string, string>) {
  await fetch('/command', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body),
  })
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
          if (!stop) setState(next)
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

export function OverviewPage() {
  const state = useHost()
  const copy = copyFor(state.language)
  return (
    <section>
      <h1 className='text-2xl font-semibold'>{copy.overview}</h1>
      <p>{copy.dashboardEmpty}</p>
      <p>{copy.widgetsLater}</p>
      <p>{copy.user}: {state.user}</p>
      <p>{copy.storage}: {state.storage}</p>
      <p>{copy.run}: {state.run}</p>
      <p>{copy.recovery}</p>
    </section>
  )
}

export function PluginsPage() {
  const state = useHost()
  const copy = copyFor(state.language)
  return (
    <section>
      <h1 className='text-2xl font-semibold'>{copy.plugins}</h1>
      {state.plugins.length === 0 ? <p>{copy.pluginsEmpty}</p> : null}
      <ul>
        {state.plugins.map((plugin) => (
          <li key={plugin.id}>
            {plugin.name} ({plugin.id}) {plugin.availability}
          </li>
        ))}
      </ul>
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
    </section>
  )
}
