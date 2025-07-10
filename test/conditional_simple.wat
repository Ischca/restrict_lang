(module
  ;; WASI imports
  (import "wasi_snapshot_preview1" "fd_write" (func $fd_write (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))

  ;; Memory
  (memory 1)
  (export "memory" (memory 0))

  ;; String constants
  (data (i32.const 0) "")

  ;; Functions
)
