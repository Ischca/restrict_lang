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
  (func $add3 (param $a i32) (param $b i32) (param $c i32) (result i32)
    local.get $a
    local.get $b
    i32.add
    local.get $c
    i32.add
  )
  (func $main (result i32)
    i32.const 10
    i32.const 20
    i32.const 30
    call $add3
  )

  ;; Export main
  (export "_start" (func $main))
)
