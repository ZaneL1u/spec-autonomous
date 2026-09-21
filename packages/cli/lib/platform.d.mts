//#region packages/cli/src/lib/platform.d.mts
export interface Platform {
  key: string;
  target: string;
  os: NodeJS.Platform;
  cpu: string;
  libc?: string;
  executable: string;
}
export declare const platforms: readonly Platform[];
export declare function platformFor(os?: NodeJS.Platform, cpu?: string, glibc?: string | undefined): Platform;
//#endregion