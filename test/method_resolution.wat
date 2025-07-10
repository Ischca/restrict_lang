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
  (func $A_process (param $self i32) (result i32)
    i32.const 100
  )
  (func $B_process (param $self i32) (result i32)
    i32.const 200
  )
  (func $main (result i32)
    i32.const 1024
    i32.const 1024
    i32.const 0
    i32.add
    i32.const 1
    i32.store
    i32.const 1024
    call $B_process
    i32.const 1024
    i32.const 1024
    i32.const 0
    i32.add
    i32.const 2
    i32.store
    i32.const 1024
    call $B_process
    i32.add
  )

  ;; Export main
  (export "_start" (func $main))
)
