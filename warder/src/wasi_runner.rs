use anyhow::{bail, Context, Result};
use std::fmt;
use std::io::{Cursor, Read, Write};
use wasmi::errors::HostError;
use wasmi::{Caller, Engine, Linker, Memory, Module, Store};

const MAX_IOVECS: i32 = 1024;
const MAX_CAPTURE_BYTES: usize = 16 * 1024 * 1024;

const ERRNO_SUCCESS: i32 = 0;
const ERRNO_BADF: i32 = 8;
const ERRNO_FAULT: i32 = 21;
const ERRNO_INVAL: i32 = 28;
const ERRNO_IO: i32 = 29;
const ERRNO_OVERFLOW: i32 = 61;

/// Host error raised by `wasi_snapshot_preview1::proc_exit` to unwind execution
/// immediately while preserving the guest's requested exit code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WasiProcExit(pub i32);

impl fmt::Display for WasiProcExit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WASI proc_exit({})", self.0)
    }
}

impl std::error::Error for WasiProcExit {}

impl HostError for WasiProcExit {}

pub enum OutputSink {
    Stream(Box<dyn Write + Send>),
    Capture { buffer: Vec<u8>, max_bytes: usize },
}

impl fmt::Debug for OutputSink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stream(_) => f.write_str("OutputSink::Stream(..)"),
            Self::Capture { buffer, max_bytes } => f
                .debug_struct("OutputSink::Capture")
                .field("len", &buffer.len())
                .field("max_bytes", max_bytes)
                .finish(),
        }
    }
}

impl OutputSink {
    fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), i32> {
        match self {
            Self::Stream(writer) => {
                writer.write_all(bytes).map_err(|_| ERRNO_IO)?;
                writer.flush().map_err(|_| ERRNO_IO)?;
                Ok(())
            }
            Self::Capture { buffer, max_bytes } => {
                if buffer
                    .len()
                    .checked_add(bytes.len())
                    .is_none_or(|total| total > *max_bytes)
                {
                    return Err(ERRNO_FAULT);
                }
                buffer.extend_from_slice(bytes);
                Ok(())
            }
        }
    }

    fn take_captured(&self) -> &[u8] {
        match self {
            Self::Stream(_) => &[],
            Self::Capture { buffer, .. } => buffer,
        }
    }
}

pub enum InputSource {
    Stream(Box<dyn Read + Send>),
    Buffered(Cursor<Vec<u8>>),
}

impl fmt::Debug for InputSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stream(_) => f.write_str("InputSource::Stream(..)"),
            Self::Buffered(cursor) => f
                .debug_struct("InputSource::Buffered")
                .field("len", &cursor.get_ref().len())
                .finish(),
        }
    }
}

impl InputSource {
    fn read_bytes(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::Stream(reader) => reader.read(buf),
            Self::Buffered(cursor) => cursor.read(buf),
        }
    }
}

/// Host state backing `wasi_snapshot_preview1` calls inside `wasmi`.
#[derive(Debug)]
pub struct WasiState {
    args: Vec<String>,
    envs: Vec<String>,
    stdin: InputSource,
    stdout: OutputSink,
    stderr: OutputSink,
}

impl WasiState {
    /// Creates a state wired to the current process's stdin, stdout, and stderr.
    pub fn inherited(args: Vec<String>, envs: Vec<(String, String)>) -> Self {
        let formatted_envs = envs
            .into_iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect();
        Self {
            args,
            envs: formatted_envs,
            stdin: InputSource::Stream(Box::new(std::io::stdin())),
            stdout: OutputSink::Stream(Box::new(std::io::stdout())),
            stderr: OutputSink::Stream(Box::new(std::io::stderr())),
        }
    }

    /// Creates an isolated state that captures stdout/stderr and feeds EOF on stdin.
    pub fn captured() -> Self {
        Self::captured_with(Vec::new(), Vec::new(), Vec::new())
    }

    /// Creates an isolated state with explicit args, env vars, and stdin bytes.
    pub fn captured_with(
        args: Vec<String>,
        envs: Vec<(String, String)>,
        stdin_bytes: Vec<u8>,
    ) -> Self {
        let formatted_envs = envs
            .into_iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect();
        Self {
            args,
            envs: formatted_envs,
            stdin: InputSource::Buffered(Cursor::new(stdin_bytes)),
            stdout: OutputSink::Capture {
                buffer: Vec::new(),
                max_bytes: MAX_CAPTURE_BYTES,
            },
            stderr: OutputSink::Capture {
                buffer: Vec::new(),
                max_bytes: MAX_CAPTURE_BYTES,
            },
        }
    }

    pub fn stdout_bytes(&self) -> &[u8] {
        self.stdout.take_captured()
    }

    pub fn stderr_bytes(&self) -> &[u8] {
        self.stderr.take_captured()
    }
}

/// Registers the `wasi_snapshot_preview1` host functions on `linker`.
pub fn register_wasi_preview1(linker: &mut Linker<WasiState>) -> Result<()> {
    linker
        .func_wrap("wasi_snapshot_preview1", "fd_write", wasi_fd_write)
        .context("Failed to register wasi_snapshot_preview1::fd_write")?;
    linker
        .func_wrap("wasi_snapshot_preview1", "fd_read", wasi_fd_read)
        .context("Failed to register wasi_snapshot_preview1::fd_read")?;
    linker
        .func_wrap("wasi_snapshot_preview1", "proc_exit", wasi_proc_exit)
        .context("Failed to register wasi_snapshot_preview1::proc_exit")?;
    linker
        .func_wrap(
            "wasi_snapshot_preview1",
            "args_sizes_get",
            wasi_args_sizes_get,
        )
        .context("Failed to register wasi_snapshot_preview1::args_sizes_get")?;
    linker
        .func_wrap("wasi_snapshot_preview1", "args_get", wasi_args_get)
        .context("Failed to register wasi_snapshot_preview1::args_get")?;
    linker
        .func_wrap(
            "wasi_snapshot_preview1",
            "environ_sizes_get",
            wasi_environ_sizes_get,
        )
        .context("Failed to register wasi_snapshot_preview1::environ_sizes_get")?;
    linker
        .func_wrap("wasi_snapshot_preview1", "environ_get", wasi_environ_get)
        .context("Failed to register wasi_snapshot_preview1::environ_get")?;
    Ok(())
}

/// Validates, instantiates, and runs a WASI Preview 1 Core Wasm module.
///
/// Returns the guest exit code (`0` on normal return or `proc_exit(0)`, or the
/// code passed to `proc_exit`) along with the final `WasiState`.
pub fn run_wasi_module(wasm: &[u8], state: WasiState) -> Result<(i32, WasiState)> {
    wasmparser::Validator::new()
        .validate_all(wasm)
        .context("Compiled WebAssembly module is invalid")?;
    let engine = Engine::default();
    let module =
        Module::new(&engine, wasm).context("Failed to load compiled WebAssembly module")?;
    let mut linker = Linker::new(&engine);
    register_wasi_preview1(&mut linker)?;

    let mut store = Store::new(&engine, state);
    let instance = match linker.instantiate_and_start(&mut store, &module) {
        Ok(instance) => instance,
        Err(error) => {
            if let Some(exit) = error.downcast_ref::<WasiProcExit>() {
                return Ok((exit.0, store.into_data()));
            }
            return Err(error).context("Failed to instantiate WebAssembly module");
        }
    };

    let exit_code = if let Ok(start) = instance.get_typed_func::<(), ()>(&store, "_start") {
        match start.call(&mut store, ()) {
            Ok(()) => 0,
            Err(error) => match error.downcast_ref::<WasiProcExit>() {
                Some(exit) => exit.0,
                None => return Err(error).context("WebAssembly execution trapped"),
            },
        }
    } else if let Ok(main_fn) = instance.get_typed_func::<(), ()>(&store, "main") {
        match main_fn.call(&mut store, ()) {
            Ok(()) => 0,
            Err(error) => match error.downcast_ref::<WasiProcExit>() {
                Some(exit) => exit.0,
                None => return Err(error).context("WebAssembly execution trapped"),
            },
        }
    } else {
        bail!("WebAssembly module does not export '_start' or 'main' entry point");
    };

    Ok((exit_code, store.into_data()))
}

fn exported_memory(caller: &mut Caller<'_, WasiState>) -> Result<Memory, i32> {
    caller
        .get_export("memory")
        .and_then(|export| export.into_memory())
        .ok_or(ERRNO_FAULT)
}

fn read_u32(memory: Memory, caller: &Caller<'_, WasiState>, offset: i32) -> Result<u32, i32> {
    let offset = usize::try_from(offset).map_err(|_| ERRNO_FAULT)?;
    let mut bytes = [0u8; 4];
    memory
        .read(caller, offset, &mut bytes)
        .map_err(|_| ERRNO_FAULT)?;
    Ok(u32::from_le_bytes(bytes))
}

fn write_u32(
    memory: Memory,
    caller: &mut Caller<'_, WasiState>,
    offset: i32,
    value: u32,
) -> Result<(), i32> {
    let offset = usize::try_from(offset).map_err(|_| ERRNO_FAULT)?;
    memory
        .write(caller, offset, &value.to_le_bytes())
        .map_err(|_| ERRNO_FAULT)
}

fn wasi_proc_exit(_caller: Caller<'_, WasiState>, code: i32) -> Result<(), wasmi::Error> {
    Err(wasmi::Error::host(WasiProcExit(code)))
}

fn wasi_fd_write(
    mut caller: Caller<'_, WasiState>,
    fd: i32,
    iovs: i32,
    iovs_len: i32,
    nwritten: i32,
) -> i32 {
    if !(0..=MAX_IOVECS).contains(&iovs_len) {
        return ERRNO_INVAL;
    }
    if fd != 1 && fd != 2 {
        return ERRNO_BADF;
    }
    let Ok(memory) = exported_memory(&mut caller) else {
        return ERRNO_FAULT;
    };

    let mut total_written: u32 = 0;
    let mut staging = Vec::new();
    for index in 0..iovs_len {
        let Some(iov) = index
            .checked_mul(8)
            .and_then(|offset| iovs.checked_add(offset))
        else {
            return ERRNO_FAULT;
        };
        let base = match read_u32(memory, &caller, iov) {
            Ok(base) => base as usize,
            Err(errno) => return errno,
        };
        let Some(len_offset) = iov.checked_add(4) else {
            return ERRNO_FAULT;
        };
        let len = match read_u32(memory, &caller, len_offset) {
            Ok(len) => len as usize,
            Err(errno) => return errno,
        };
        if staging
            .len()
            .checked_add(len)
            .is_none_or(|total| total > MAX_CAPTURE_BYTES)
        {
            return ERRNO_FAULT;
        }
        let start = staging.len();
        staging.resize(start + len, 0);
        if memory.read(&caller, base, &mut staging[start..]).is_err() {
            return ERRNO_FAULT;
        }
        let Ok(chunk_len) = u32::try_from(len) else {
            return ERRNO_OVERFLOW;
        };
        let Some(next_total) = total_written.checked_add(chunk_len) else {
            return ERRNO_OVERFLOW;
        };
        total_written = next_total;
    }

    let write_result = match fd {
        1 => caller.data_mut().stdout.write_bytes(&staging),
        2 => caller.data_mut().stderr.write_bytes(&staging),
        _ => Err(ERRNO_BADF),
    };
    if let Err(errno) = write_result {
        return errno;
    }

    if write_u32(memory, &mut caller, nwritten, total_written).is_err() {
        return ERRNO_FAULT;
    }
    ERRNO_SUCCESS
}

fn wasi_fd_read(
    mut caller: Caller<'_, WasiState>,
    fd: i32,
    iovs: i32,
    iovs_len: i32,
    nread: i32,
) -> i32 {
    if !(0..=MAX_IOVECS).contains(&iovs_len) {
        return ERRNO_INVAL;
    }
    if fd != 0 {
        return ERRNO_BADF;
    }
    let Ok(memory) = exported_memory(&mut caller) else {
        return ERRNO_FAULT;
    };

    let mut total_read: u32 = 0;
    for index in 0..iovs_len {
        let Some(iov) = index
            .checked_mul(8)
            .and_then(|offset| iovs.checked_add(offset))
        else {
            return ERRNO_FAULT;
        };
        let base = match read_u32(memory, &caller, iov) {
            Ok(base) => base as usize,
            Err(errno) => return errno,
        };
        let Some(len_offset) = iov.checked_add(4) else {
            return ERRNO_FAULT;
        };
        let len = match read_u32(memory, &caller, len_offset) {
            Ok(len) => len as usize,
            Err(errno) => return errno,
        };
        if len == 0 {
            continue;
        }

        let mut buf = vec![0u8; len];
        let read_count = match caller.data_mut().stdin.read_bytes(&mut buf) {
            Ok(n) => n,
            Err(_) => return ERRNO_IO,
        };
        if read_count > 0 && memory.write(&mut caller, base, &buf[..read_count]).is_err() {
            return ERRNO_FAULT;
        }
        let Ok(chunk_read) = u32::try_from(read_count) else {
            return ERRNO_OVERFLOW;
        };
        let Some(next_total) = total_read.checked_add(chunk_read) else {
            return ERRNO_OVERFLOW;
        };
        total_read = next_total;
        if read_count < len {
            break;
        }
    }

    if write_u32(memory, &mut caller, nread, total_read).is_err() {
        return ERRNO_FAULT;
    }
    ERRNO_SUCCESS
}

fn wasi_args_sizes_get(
    mut caller: Caller<'_, WasiState>,
    argc_ptr: i32,
    argv_buf_size_ptr: i32,
) -> i32 {
    let Ok(memory) = exported_memory(&mut caller) else {
        return ERRNO_FAULT;
    };
    write_cstr_array_sizes(
        memory,
        &mut caller,
        |state| &state.args,
        argc_ptr,
        argv_buf_size_ptr,
    )
}

fn wasi_args_get(mut caller: Caller<'_, WasiState>, argv_ptr: i32, argv_buf_ptr: i32) -> i32 {
    let Ok(memory) = exported_memory(&mut caller) else {
        return ERRNO_FAULT;
    };
    write_cstr_array(
        memory,
        &mut caller,
        |state| state.args.clone(),
        argv_ptr,
        argv_buf_ptr,
    )
}

fn wasi_environ_sizes_get(
    mut caller: Caller<'_, WasiState>,
    environc_ptr: i32,
    environ_buf_size_ptr: i32,
) -> i32 {
    let Ok(memory) = exported_memory(&mut caller) else {
        return ERRNO_FAULT;
    };
    write_cstr_array_sizes(
        memory,
        &mut caller,
        |state| &state.envs,
        environc_ptr,
        environ_buf_size_ptr,
    )
}

fn wasi_environ_get(
    mut caller: Caller<'_, WasiState>,
    environ_ptr: i32,
    environ_buf_ptr: i32,
) -> i32 {
    let Ok(memory) = exported_memory(&mut caller) else {
        return ERRNO_FAULT;
    };
    write_cstr_array(
        memory,
        &mut caller,
        |state| state.envs.clone(),
        environ_ptr,
        environ_buf_ptr,
    )
}

fn write_cstr_array_sizes(
    memory: Memory,
    caller: &mut Caller<'_, WasiState>,
    items: fn(&WasiState) -> &[String],
    count_ptr: i32,
    buf_size_ptr: i32,
) -> i32 {
    let slice = items(caller.data());
    let Ok(count) = u32::try_from(slice.len()) else {
        return ERRNO_OVERFLOW;
    };
    let mut total_bytes: u32 = 0;
    for item in slice {
        let Some(with_nul) = item
            .len()
            .checked_add(1)
            .and_then(|n| u32::try_from(n).ok())
        else {
            return ERRNO_OVERFLOW;
        };
        let Some(next) = total_bytes.checked_add(with_nul) else {
            return ERRNO_OVERFLOW;
        };
        total_bytes = next;
    }

    if write_u32(memory, caller, count_ptr, count).is_err() {
        return ERRNO_FAULT;
    }
    if write_u32(memory, caller, buf_size_ptr, total_bytes).is_err() {
        return ERRNO_FAULT;
    }
    ERRNO_SUCCESS
}

fn write_cstr_array(
    memory: Memory,
    caller: &mut Caller<'_, WasiState>,
    items: fn(&WasiState) -> Vec<String>,
    ptrs_base: i32,
    buf_base: i32,
) -> i32 {
    let Ok(mut ptr_cursor) = u32::try_from(ptrs_base) else {
        return ERRNO_FAULT;
    };
    let Ok(mut buf_cursor) = u32::try_from(buf_base) else {
        return ERRNO_FAULT;
    };

    let entries = items(caller.data());
    for entry in entries {
        if memory
            .write(&mut *caller, ptr_cursor as usize, &buf_cursor.to_le_bytes())
            .is_err()
        {
            return ERRNO_FAULT;
        }
        let Some(next_ptr) = ptr_cursor.checked_add(4) else {
            return ERRNO_OVERFLOW;
        };
        ptr_cursor = next_ptr;

        let mut bytes = entry.into_bytes();
        bytes.push(0);
        if memory
            .write(&mut *caller, buf_cursor as usize, &bytes)
            .is_err()
        {
            return ERRNO_FAULT;
        }
        let Ok(len) = u32::try_from(bytes.len()) else {
            return ERRNO_OVERFLOW;
        };
        let Some(next_buf) = buf_cursor.checked_add(len) else {
            return ERRNO_OVERFLOW;
        };
        buf_cursor = next_buf;
    }
    ERRNO_SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_start_and_captures_stdout_and_stderr() {
        let wasm = wat::parse_str(
            r#"(module
                (import "wasi_snapshot_preview1" "fd_write"
                    (func $fd_write (param i32 i32 i32 i32) (result i32)))
                (memory (export "memory") 1)
                (data (i32.const 0) "\20\00\00\00\04\00\00\00")
                (data (i32.const 8) "\24\00\00\00\04\00\00\00")
                (data (i32.const 32) "out\0aerr\0a")
                (func (export "_start")
                    i32.const 1
                    i32.const 0
                    i32.const 1
                    i32.const 16
                    call $fd_write
                    drop
                    i32.const 2
                    i32.const 8
                    i32.const 1
                    i32.const 16
                    call $fd_write
                    drop)
            )"#,
        )
        .unwrap();

        let (code, state) = run_wasi_module(&wasm, WasiState::captured()).unwrap();
        assert_eq!(code, 0);
        assert_eq!(state.stdout_bytes(), b"out\n");
        assert_eq!(state.stderr_bytes(), b"err\n");
    }

    #[test]
    fn proc_exit_propagates_zero_and_non_zero_codes_and_unwinds_immediately() {
        let wasm = wat::parse_str(
            r#"(module
                (import "wasi_snapshot_preview1" "fd_write"
                    (func $fd_write (param i32 i32 i32 i32) (result i32)))
                (import "wasi_snapshot_preview1" "proc_exit"
                    (func $proc_exit (param i32)))
                (memory (export "memory") 1)
                (data (i32.const 0) "\20\00\00\00\05\00\00\00")
                (data (i32.const 32) "after")
                (func (export "_start")
                    i32.const 42
                    call $proc_exit
                    i32.const 1
                    i32.const 0
                    i32.const 1
                    i32.const 16
                    call $fd_write
                    drop)
            )"#,
        )
        .unwrap();

        let (code, state) = run_wasi_module(&wasm, WasiState::captured()).unwrap();
        assert_eq!(code, 42);
        assert!(
            state.stdout_bytes().is_empty(),
            "proc_exit must unwind immediately before subsequent writes"
        );

        let zero_wasm = wat::parse_str(
            r#"(module
                (import "wasi_snapshot_preview1" "proc_exit"
                    (func $proc_exit (param i32)))
                (func (export "_start")
                    i32.const 0
                    call $proc_exit)
            )"#,
        )
        .unwrap();
        let (zero_code, _) = run_wasi_module(&zero_wasm, WasiState::captured()).unwrap();
        assert_eq!(zero_code, 0);
    }

    #[test]
    fn forwards_cli_args_env_vars_and_stdin() {
        let wasm = wat::parse_str(
            r#"(module
                (import "wasi_snapshot_preview1" "args_sizes_get"
                    (func $args_sizes_get (param i32 i32) (result i32)))
                (import "wasi_snapshot_preview1" "args_get"
                    (func $args_get (param i32 i32) (result i32)))
                (import "wasi_snapshot_preview1" "environ_sizes_get"
                    (func $environ_sizes_get (param i32 i32) (result i32)))
                (import "wasi_snapshot_preview1" "environ_get"
                    (func $environ_get (param i32 i32) (result i32)))
                (import "wasi_snapshot_preview1" "fd_read"
                    (func $fd_read (param i32 i32 i32 i32) (result i32)))
                (import "wasi_snapshot_preview1" "fd_write"
                    (func $fd_write (param i32 i32 i32 i32) (result i32)))
                (memory (export "memory") 1)
                (func (export "_start")
                    ;; args_sizes_get -> [0..4]=argc, [4..8]=buf_size
                    i32.const 0
                    i32.const 4
                    call $args_sizes_get
                    drop
                    ;; args_get(ptrs=64, buf=128)
                    i32.const 64
                    i32.const 128
                    call $args_get
                    drop
                    ;; environ_sizes_get -> [8..12]=envc, [12..16]=buf_size
                    i32.const 8
                    i32.const 12
                    call $environ_sizes_get
                    drop
                    ;; environ_get(ptrs=256, buf=320)
                    i32.const 256
                    i32.const 320
                    call $environ_get
                    drop
                    ;; fd_read(0, iov=(512: base=600, len=16), 1, nread=520)
                    i32.const 512
                    i32.const 600
                    i32.store
                    i32.const 516
                    i32.const 16
                    i32.store
                    i32.const 0
                    i32.const 512
                    i32.const 1
                    i32.const 520
                    call $fd_read
                    drop
                    ;; Echo arg1 (ptr at 68), env0 (ptr at 256), and stdin (600, len at 520)
                    ;; iovec0: arg1 (len=5: "hello")
                    i32.const 700
                    i32.const 68
                    i32.load
                    i32.store
                    i32.const 704
                    i32.const 5
                    i32.store
                    ;; iovec1: env0 (len=7: "KEY=VAL")
                    i32.const 708
                    i32.const 256
                    i32.load
                    i32.store
                    i32.const 712
                    i32.const 7
                    i32.store
                    ;; iovec2: stdin (len=[520])
                    i32.const 716
                    i32.const 600
                    i32.store
                    i32.const 720
                    i32.const 520
                    i32.load
                    i32.store
                    i32.const 1
                    i32.const 700
                    i32.const 3
                    i32.const 740
                    call $fd_write
                    drop)
            )"#,
        )
        .unwrap();

        let state = WasiState::captured_with(
            vec!["app.wasm".to_string(), "hello".to_string()],
            vec![("KEY".to_string(), "VAL".to_string())],
            b":stdin".to_vec(),
        );
        let (code, state) = run_wasi_module(&wasm, state).unwrap();
        assert_eq!(code, 0);
        assert_eq!(state.stdout_bytes(), b"helloKEY=VAL:stdin");
    }
}
