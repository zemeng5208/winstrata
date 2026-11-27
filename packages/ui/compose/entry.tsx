import { createRoot } from 'react-dom/client'
import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  RouterProvider,
} from '@tanstack/react-router'
import { FilterPage } from './filter-page'
import { ScriptPage } from './script-page'
import { TagPage } from './tag-page'
import { OverviewPage, PluginsPage, ServicePage, SettingsPage, Shell } from './shell'

const rootRoute = createRootRoute({ component: Shell })
const overviewRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/',
  component: OverviewPage,
})
const pluginsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/plugins',
  component: PluginsPage,
})
const serviceRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/service',
  component: ServicePage,
})
const settingsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/settings',
  component: SettingsPage,
})
const scriptsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/scripts',
  component: ScriptPage,
})
const filtersRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/filters',
  component: FilterPage,
})
const tagsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/tags',
  component: TagPage,
})
const routeTree = rootRoute.addChildren([
  overviewRoute,
  pluginsRoute,
  serviceRoute,
  settingsRoute,
  scriptsRoute,
  filtersRoute,
  tagsRoute,
])
const router = createRouter({
  routeTree,
  history: createMemoryHistory({ initialEntries: ['/'] }),
})

const root = document.getElementById('root')
if (!root) throw new Error('missing root')
createRoot(root).render(<RouterProvider router={router} />)
