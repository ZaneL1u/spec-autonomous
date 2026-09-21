// Compatibility baseline. uv digests are the official GitHub release asset
// SHA256 digests, reviewed 2026-09-11. Updates require installation smoke tests.
export const versions = Object.freeze({ openspec: '1.13.0', speckit: '1.0.6', uv: '0.12.13' });
export const uvHashes: Readonly<Record<string, string>> = Object.freeze({
  'darwin-arm64': '7e6ddb9316acc00f2296c82ff4d99977870ee34b2f0ddcae9444d714db9364ed',
  'darwin-x64': '5e287ef61cb6a9b61b3a83fef124fd143e400468a7dac794230147a810e17119',
  'linux-arm64': '2eaa5d94f5db7b3a1a092156b9420459e42ab0217d917fe74a876309cef9b5e9',
  'linux-x64': '745765a3b6e360ad76743599ae5c42e9278c7edf8bbff9fc76d05bf2623a04dd',
  'win32-arm64': '1efb2654b06e7063d4ac1fc9d49a9bda9a6704d82f035b589a2751a592f14151',
  'win32-x64': 'a86c9dc7bad9b03f388583b7187c05fe9951c2e0d392217e8fd43d97787f6ec2',
});
