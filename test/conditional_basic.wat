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
  (func $is_positive (param $x i32) (result i32)
    local.get $x
    i32.const 0
    i32.gt_s
    i32.const 0
    i32.ne
    (if (result i32)
      (then
    i32.const 1
      )
          (else
    i32.const 0
          )
    )
  )
  (func $main (result i32)
    (local $result i32)
    i32.const 42
    call $is_positive
    local.set $result
    local.get $result
  )

  ;; Export main
  (export "_start" (func $main))
)
