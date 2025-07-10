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
  (func $inc (param $x i32) (result i32)
    local.get $x
    i32.const 1
    i32.add
  )
  (func $main (result i32)
    i32.const 42
    call $inc
  )

  ;; Export main
  (export "_start" (func $main))
)
