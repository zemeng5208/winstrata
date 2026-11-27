export type Language = 'en' | 'zh-CN'

export type Copy = {
  product: string
  subtitle: string
  skip: string
  overview: string
  plugins: string
  service: string
  settings: string
  navigation: string
  theme: string
  whiteAcrylic: string
  blackAcrylic: string
  liquidGlass: string
  themeUnset: string
  language: string
  english: string
  chinese: string
  close: string
  closeAsk: string
  closeTray: string
  closeExit: string
  closePending: string
  dashboardEmpty: string
  widgetsLater: string
  storage: string
  run: string
  user: string
  pluginsEmpty: string
  serviceDown: string
  officialGui: string
  recovery: string
}

const en: Copy = {
  product: 'WinStrata',
  subtitle: 'Official default GUI',
  skip: 'Skip to Main',
  overview: 'Overview',
  plugins: 'Plugins',
  service: 'Service',
  settings: 'Settings',
  navigation: 'Navigation',
  theme: 'Theme',
  whiteAcrylic: 'White Acrylic',
  blackAcrylic: 'Black Acrylic',
  liquidGlass: 'Liquid Glass',
  themeUnset: 'No theme is selected. The initial theme is not confirmed.',
  language: 'Language',
  english: 'English',
  chinese: 'Simplified Chinese',
  close: 'When the window closes',
  closeAsk: 'Ask',
  closeTray: 'Keep running in the tray',
  closeExit: 'Exit',
  closePending: 'Close the window or keep WinStrata in the tray. Collection is not running in this window.',
  dashboardEmpty: 'The dashboard has no widgets.',
  widgetsLater: 'Widget layout is a later plan. This page does not invent sample charts.',
  storage: 'Storage',
  run: 'Run',
  user: 'User',
  pluginsEmpty: 'Plugin reports were not read.',
  serviceDown: 'The service did not answer.',
  officialGui: 'This official GUI stays installed.',
  recovery: 'It is the default and the recovery interface.',
}

const zh: Copy = {
  product: 'WinStrata',
  subtitle: '官方默认界面',
  skip: '跳到主内容',
  overview: '概览',
  plugins: '插件',
  service: '服务',
  settings: '设置',
  navigation: '导航',
  theme: '主题',
  whiteAcrylic: '白色圆角亚克力',
  blackAcrylic: '黑色圆角亚克力',
  liquidGlass: '液态玻璃',
  themeUnset: '尚未选择主题。初始主题未经确认。',
  language: '语言',
  english: '英文',
  chinese: '简体中文',
  close: '关闭窗口时',
  closeAsk: '询问',
  closeTray: '继续留在托盘',
  closeExit: '退出',
  closePending: '关闭窗口，或让 WinStrata 留在托盘。这个窗口里没有在采集。',
  dashboardEmpty: '仪表盘没有小组件。',
  widgetsLater: '小组件布局属于后续计划。此页不编造示例图表。',
  storage: '存储',
  run: '运行',
  user: '用户',
  pluginsEmpty: '没有读到插件报告。',
  serviceDown: '服务没有应答。',
  officialGui: '这个官方界面保持安装。',
  recovery: '它是默认界面，也是恢复界面。',
}

export function copyFor(language: Language | null): Copy {
  return language === 'zh-CN' ? zh : en
}
