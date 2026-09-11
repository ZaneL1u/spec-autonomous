export const platforms = [
  { key: 'darwin-arm64', target: 'aarch64-apple-darwin', os: 'darwin', cpu: 'arm64', executable: 'spec-autonomous' },
  { key: 'darwin-x64', target: 'x86_64-apple-darwin', os: 'darwin', cpu: 'x64', executable: 'spec-autonomous' },
  { key: 'linux-arm64', target: 'aarch64-unknown-linux-gnu', os: 'linux', cpu: 'arm64', libc: 'glibc', executable: 'spec-autonomous' },
  { key: 'linux-x64', target: 'x86_64-unknown-linux-gnu', os: 'linux', cpu: 'x64', libc: 'glibc', executable: 'spec-autonomous' },
  { key: 'win32-arm64', target: 'aarch64-pc-windows-msvc', os: 'win32', cpu: 'arm64', executable: 'spec-autonomous.exe' },
  { key: 'win32-x64', target: 'x86_64-pc-windows-msvc', os: 'win32', cpu: 'x64', executable: 'spec-autonomous.exe' },
];

export function platformFor(os = process.platform, cpu = process.arch, glibc = process.report?.getReport().header.glibcVersionRuntime) {
  const platform = platforms.find((p) => p.os === os && p.cpu === cpu);
  if (!platform) throw new Error(`Unsupported platform: ${os}-${cpu}. See the supported target matrix.`);
  if (os === 'linux' && !glibc) throw new Error('Linux musl is not yet supported; use a glibc host or build from Rust source.');
  return platform;
}
