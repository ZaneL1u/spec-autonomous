// `catch` binds `unknown`, but this CLI routinely branches on POSIX error codes
// and message prefixes. These readers keep that narrowing in one place.
export function errorCode(error: unknown): string | undefined {
  return typeof error === 'object' && error !== null && 'code' in error ? String((error as { code: unknown }).code) : undefined;
}

export function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
