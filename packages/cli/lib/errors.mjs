//#region packages/cli/src/lib/errors.mts
function errorCode(error) {
	return typeof error === "object" && error !== null && "code" in error ? String(error.code) : void 0;
}
function errorMessage(error) {
	return error instanceof Error ? error.message : String(error);
}
//#endregion
export { errorCode, errorMessage };
