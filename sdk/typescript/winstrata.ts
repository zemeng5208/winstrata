/** WinStrata TypeScript 插件可以使用的宿主类型。实现位于 JavaScript SDK。 */

export interface PluginVersion {
  major: number;
  minor: number;
  patch: number;
}

export interface LanguageModule {
  VALUE?: string;
}

export interface WinStrataHost {
  loadModule(name: string): Promise<LanguageModule>;
}

declare global {
  var winstrata: WinStrataHost;
}

export {};
