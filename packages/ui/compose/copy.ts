export type Language = 'en' | 'zh-CN'

export type Copy = {
  product: string
  subtitle: string
  skip: string
  overview: string
  plugins: string
  scripts: string
  filters: string
  filtersTitle: string
  filtersBody: string
  filtersField: string
  filtersValue: string
  filtersSave: string
  filtersShow: string
  filtersIdle: string
  filtersInclude: string
  filtersExclude: string
  scriptsTitle: string
  scriptsBody: string
  scriptsSource: string
  scriptsName: string
  scriptsPrefix: string
  scriptsCheck: string
  scriptsDiagnose: string
  scriptsHighlight: string
  scriptsComplete: string
  scriptsSave: string
  scriptsIdle: string
  scriptsPresets: string
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
  onboardingTitle: string
  onboardingBody: string
  locationLabel: string
  useLocation: string
  skipLanguage: string
  skipStep: string
  pluginsBody: string
  rescue: string
  rescueBody: string
  scopeTitle: string
  scopeBody: string
  rootsLabel: string
  typesLabel: string
  processesLabel: string
  softwareLabel: string
  registryLabel: string
  interfacesLabel: string
  watchProcesses: string
  saveScope: string
  retentionLabel: string
  retentionBody: string
  bootLabel: string
  encryptionLabel: string
  encryptionBody: string
  encryptionPassword: string
  autoLock: string
  mcpLabel: string
  baselineTitle: string
  baselineBody: string
  buildBaseline: string
  modeTitle: string
  modeBody: string
  live: string
  history: string
  pause: string
  resume: string
  stopCollection: string
  paused: string
  collectors: string
  drivers: string
  markers: string
  emptyMarkers: string
  noLossButton: string
  step: string
}

const en: Copy = {
  product: 'WinStrata',
  subtitle: 'Official default GUI',
  skip: 'Skip to Main',
  overview: 'Overview',
  plugins: 'Plugins',
  scripts: 'Scripts',
  filters: 'Filters',
  filtersTitle: 'StrataFilter',
  filtersBody: 'This page does not enable the plugin and does not grant a permission. A command is forwarded only while the filter is already running. Self writes stay suppressed when the plugin is not running.',
  filtersField: 'Field',
  filtersValue: 'Value',
  filtersSave: 'Save rule',
  filtersShow: 'Show counts',
  filtersIdle: 'The filter is not running.',
  filtersInclude: 'Include',
  filtersExclude: 'Exclude',
  scriptsTitle: 'StrataScript',
  scriptsBody: 'This page does not enable the plugin and does not grant a permission. Presets stay empty. A command is forwarded only while the workbench is already running.',
  scriptsSource: 'Source',
  scriptsName: 'Script name',
  scriptsPrefix: 'Completion prefix',
  scriptsCheck: 'Check presets',
  scriptsDiagnose: 'Diagnose',
  scriptsHighlight: 'Highlight',
  scriptsComplete: 'Complete',
  scriptsSave: 'Save source',
  scriptsIdle: 'The workbench is not running.',
  scriptsPresets: 'Presets',
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
  closePending: 'Close the window or keep WinStrata in the tray. Exit stops live capture, flushes, and records the gap before the window closes. A driver is released only when this session subscribed one.',
  dashboardEmpty: 'The dashboard has no widgets.',
  widgetsLater: 'Widget layout is a later plan. This page does not invent sample charts.',
  storage: 'Storage',
  run: 'Run',
  user: 'User',
  pluginsEmpty: 'Plugin reports were not read.',
  serviceDown: 'The service did not answer.',
  officialGui: 'This official GUI stays installed.',
  recovery: 'It is the default and the recovery interface.',
  onboardingTitle: 'First collection',
  onboardingBody: 'Choose a real location, the plugins to opt in, and the scope to watch. Opt-in does not start a collector. A missing location is not replaced.',
  locationLabel: 'Storage location',
  useLocation: 'Use this location',
  skipLanguage: 'Skip language',
  skipStep: 'Skip this step',
  pluginsBody: 'These choices stay installed. Their collectors are not started from this page.',
  rescue: 'Rescue',
  rescueBody: 'Rescue stays out of the batch and remains off until you choose it separately.',
  scopeTitle: 'Scope',
  scopeBody: 'Only selected directories, types, processes, software, registry keys, and interfaces are eligible.',
  rootsLabel: 'Directories, one absolute path per line',
  typesLabel: 'File extensions, blank for every type',
  processesLabel: 'Process names',
  softwareLabel: 'Software names',
  registryLabel: 'Current-user registry keys',
  interfacesLabel: 'Interfaces',
  watchProcesses: 'Watch processes under the selected directories',
  saveScope: 'Save scope',
  retentionLabel: 'Retention days',
  retentionBody: 'Leave this blank for indefinite retention.',
  bootLabel: 'Start at login',
  encryptionLabel: 'Encryption',
  encryptionBody: 'Choose encryption before the baseline. It cannot be added after the first record.',
  encryptionPassword: 'Encryption password',
  autoLock: 'Auto-lock seconds, 0 to keep it unlocked',
  mcpLabel: 'Opt in to MCP',
  baselineTitle: 'Baseline',
  baselineBody: 'The baseline records file metadata in the selected scope and labels it reconstructed. Software, registry, environment, services, drivers, and tasks are not scanned.',
  buildBaseline: 'Build the selected baseline',
  modeTitle: 'Mode',
  modeBody: 'History and stop flush and record a gap. They ask the service to unload a driver only when this session subscribed one. Pause does not.',
  live: 'Live',
  history: 'History',
  pause: 'Pause',
  resume: 'Resume',
  stopCollection: 'Stop',
  paused: 'Paused',
  collectors: 'Collectors',
  drivers: 'Drivers',
  markers: 'Markers',
  emptyMarkers: 'No mode markers yet.',
  noLossButton: 'This page does not invent a loss, a failure, or a paused gap.',
  step: 'Step',
}

const zh: Copy = {
  product: 'WinStrata',
  subtitle: '官方默认界面',
  skip: '跳到主内容',
  overview: '概览',
  plugins: '插件',
  scripts: '脚本',
  filters: '过滤',
  filtersTitle: 'StrataFilter',
  filtersBody: '此页不启用插件，也不授予权限。只有过滤器已经在运行时，命令才会被转发。插件没有运行时，自身写入仍然被抑制。',
  filtersField: '字段',
  filtersValue: '值',
  filtersSave: '保存规则',
  filtersShow: '查看计数',
  filtersIdle: '过滤器没有在运行。',
  filtersInclude: '包含',
  filtersExclude: '排除',
  scriptsTitle: 'StrataScript',
  scriptsBody: '此页不启用插件，也不授予权限。预设保持为空。只有工作台已经在运行时，命令才会被转发。',
  scriptsSource: '源码',
  scriptsName: '脚本名',
  scriptsPrefix: '补全前缀',
  scriptsCheck: '检查预设',
  scriptsDiagnose: '诊断',
  scriptsHighlight: '着色',
  scriptsComplete: '补全',
  scriptsSave: '保存源码',
  scriptsIdle: '工作台没有在运行。',
  scriptsPresets: '预设',
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
  closePending: '关闭窗口，或让 WinStrata 留在托盘。退出会先停止实时采集、落盘并记录缺口，然后关闭窗口。只有这个会话实际订阅了驱动时才会释放。',
  dashboardEmpty: '仪表盘没有小组件。',
  widgetsLater: '小组件布局属于后续计划。此页不编造示例图表。',
  storage: '存储',
  run: '运行',
  user: '用户',
  pluginsEmpty: '没有读到插件报告。',
  serviceDown: '服务没有应答。',
  officialGui: '这个官方界面保持安装。',
  recovery: '它是默认界面，也是恢复界面。',
  onboardingTitle: '首次采集',
  onboardingBody: '选择真实的位置、要启用的插件和监视范围。启用选择不会启动采集器。缺失的位置不会被替换。',
  locationLabel: '存储位置',
  useLocation: '使用这个位置',
  skipLanguage: '跳过语言',
  skipStep: '跳过这一步',
  pluginsBody: '这些身份保持安装。此页不会启动它们的采集器。',
  rescue: '救援',
  rescueBody: '救援不进入批量启用，除非单独选择，否则保持关闭。',
  scopeTitle: '范围',
  scopeBody: '只有选中的目录、类型、进程、软件、注册表键和接口才会进入记录。',
  rootsLabel: '目录，每行一个绝对路径',
  typesLabel: '文件扩展名，留空表示全部类型',
  processesLabel: '进程名',
  softwareLabel: '软件名',
  registryLabel: '当前用户的注册表键',
  interfacesLabel: '接口',
  watchProcesses: '监视所选目录下的进程',
  saveScope: '保存范围',
  retentionLabel: '保留天数',
  retentionBody: '留空表示无限期保留。',
  bootLabel: '登录时启动',
  encryptionLabel: '加密',
  encryptionBody: '请在基线之前选择加密。第一条记录写入后不能再补上。',
  encryptionPassword: '加密口令',
  autoLock: '自动锁定秒数，0 表示不自动锁定',
  mcpLabel: '启用 MCP',
  baselineTitle: '基线',
  baselineBody: '基线只记录所选范围内的文件元数据，并标记为重建。软件、注册表、环境、服务、驱动和任务不会扫描。',
  buildBaseline: '建立所选基线',
  modeTitle: '模式',
  modeBody: '历史和停止会落盘并记录缺口。只有这个会话订阅了驱动时，才会要求服务卸载。暂停不会。',
  live: '实时',
  history: '历史',
  pause: '暂停',
  resume: '继续',
  stopCollection: '停止',
  paused: '已暂停',
  collectors: '采集器',
  drivers: '驱动',
  markers: '标记',
  emptyMarkers: '还没有模式标记。',
  noLossButton: '此页不会编造丢失、失败或暂停缺口。',
  step: '步骤',
}

export function copyFor(language: Language | null): Copy {
  return language === 'zh-CN' ? zh : en
}
