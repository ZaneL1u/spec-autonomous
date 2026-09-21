//#region packages/cli/src/lib/locale.d.mts
export type Locale = 'en' | 'zh-CN';
export type Catalog = Record<string, string>;
export type SystemLanguageReader = (command: string, args: string[], options: {
  encoding: 'utf8';
  timeout: number;
  maxBuffer: number;
  windowsHide: boolean;
  shell: boolean;
}) => {
  status: number | null;
  stdout: string;
};
export interface DetectLocaleOptions {
  explicit?: string | undefined;
  env?: NodeJS.ProcessEnv;
  intl?: boolean;
  system?: () => string[];
}
export declare const withLocale: <T>(locale: Locale, action: () => T) => T;
export declare function systemLanguages(platform?: NodeJS.Platform, run?: SystemLanguageReader): string[];
export declare function normalizeLocale(value: string | undefined | null): Locale | null;
export declare function detectLocale({ explicit, env, intl, system }?: DetectLocaleOptions): Locale;
export declare function catalog(locale?: Locale): Catalog;
export declare function message(key: string, locale?: Locale, variables?: Record<string, unknown>): string;
export declare function languageArg(argv?: string[]): string | undefined;
export declare function localizeError(error: unknown, locale?: Locale): string;
//#endregion