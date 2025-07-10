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
  (func $Enemy_attack (param $self i32) (param $tgt i32) (result i32)
    local.get $self
    i32.const 4
    i32.add
    i32.load
    local.get $tgt
    i32.add
  )
  (func $main (result i32)
    (local $damage i32)
    i32.const 1024
    i32.const 1024
    i32.const 0
    i32.add
    i32.const 500
    i32.store
    i32.const 1024
    i32.const 4
    i32.add
    i32.const 50
    i32.store
    i32.const 1024
    i32.const 10
    call $Enemy_attack
    local.set $damage
    local.get $damage
  )

  ;; Export main
  (export "_start" (func $main))
)
