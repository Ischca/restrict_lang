/* tslint:disable */
/* eslint-disable */
/**
 * Get all symbols in the source code
 */
export function get_symbols(source: string): any;
export function parse_only(source: string): any;
export function compile_restrict_lang(source: string): any;
/**
 * Compile with rich error information
 */
export function compile_with_diagnostics(source: string): any;
/**
 * Type check only, returning symbol information
 */
export function type_check_only(source: string): any;
/**
 * Get inlay hints for the source code
 */
export function get_inlay_hints(source: string): any;
/**
 * Get Rust-style formatted error output
 */
export function get_formatted_errors(source: string): string;
/**
 * Get semantic tokens for syntax highlighting
 */
export function get_semantic_tokens(source: string): any;
export function init(): void;
export function lex_only(source: string): any;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly compile_restrict_lang: (a: number, b: number) => any;
  readonly compile_with_diagnostics: (a: number, b: number) => any;
  readonly get_formatted_errors: (a: number, b: number) => [number, number];
  readonly get_inlay_hints: (a: number, b: number) => any;
  readonly get_semantic_tokens: (a: number, b: number) => any;
  readonly get_symbols: (a: number, b: number) => any;
  readonly lex_only: (a: number, b: number) => any;
  readonly parse_only: (a: number, b: number) => any;
  readonly type_check_only: (a: number, b: number) => any;
  readonly init: () => void;
  readonly __wbindgen_free: (a: number, b: number, c: number) => void;
  readonly __wbindgen_malloc: (a: number, b: number) => number;
  readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
  readonly __wbindgen_export_3: WebAssembly.Table;
  readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;
/**
* Instantiates the given `module`, which can either be bytes or
* a precompiled `WebAssembly.Module`.
*
* @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
*
* @returns {InitOutput}
*/
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
* If `module_or_path` is {RequestInfo} or {URL}, makes a request and
* for everything else, calls `WebAssembly.instantiate` directly.
*
* @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
*
* @returns {Promise<InitOutput>}
*/
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
