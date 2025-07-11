(module
  ;; WASI imports
  (import "wasi_snapshot_preview1" "fd_write" (func $fd_write (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))

  ;; Memory
  (memory 1)
  (export "memory" (memory 0))

  ;; String constants

  ;; Built-in functions
  (func $println (param $str i32)
    (local $len i32)
    (local $iov_base i32)
    (local $iov_len i32)
    (local $nwritten i32)
    
    ;; Read string length from memory (first 4 bytes)
    local.get $str
    i32.load
    local.set $len
    
    ;; Prepare iovec structure at memory address 0
    ;; iov_base = str + 4 (skip length prefix)
    i32.const 0
    local.get $str
    i32.const 4
    i32.add
    i32.store
    
    ;; iov_len = string length
    i32.const 4
    local.get $len
    i32.store
    
    ;; Add newline to iovec
    ;; Store newline at address 16
    i32.const 16
    i32.const 10  ;; '\n'
    i32.store8
    
    ;; Second iovec for newline
    i32.const 8   ;; second iovec base
    i32.const 16  ;; address of newline
    i32.store
    
    i32.const 12  ;; second iovec len
    i32.const 1   ;; length of newline
    i32.store
    
    ;; Call fd_write
    i32.const 1   ;; stdout
    i32.const 0   ;; iovs
    i32.const 2   ;; iovs_len (2 iovecs)
    i32.const 20  ;; nwritten (output param)
    call $fd_write
    drop
  )

  ;; Arena allocator functions
  (global $current_arena (mut i32) (i32.const 0))

  (func $arena_init (param $start i32) (result i32)
    ;; Initialize arena header
    ;; Store start address at offset 0
    local.get $start
    local.get $start
    i32.store
    ;; Store current address at offset 4 (start + 8 for header)
    local.get $start
    i32.const 4
    i32.add
    local.get $start
    i32.const 8
    i32.add
    i32.store
    ;; Return arena header address
    local.get $start
  )
  (func $arena_alloc (param $arena i32) (param $size i32) (result i32)
    (local $current i32)
    (local $aligned_size i32)
    (local $new_current i32)
    
    ;; Load current pointer
    local.get $arena
    i32.const 4
    i32.add
    i32.load
    local.set $current
    
    ;; Align size to 4 bytes
    local.get $size
    i32.const 3
    i32.add
    i32.const -4
    i32.and
    local.set $aligned_size
    
    ;; Calculate new current
    local.get $current
    local.get $aligned_size
    i32.add
    local.set $new_current
    
    ;; TODO: Add bounds checking
    
    ;; Update current pointer
    local.get $arena
    i32.const 4
    i32.add
    local.get $new_current
    i32.store
    
    ;; Return allocated address
    local.get $current
  )
  (func $arena_reset (param $arena i32)
    ;; Reset current to start + 8 (after header)
    local.get $arena
    i32.const 4
    i32.add
    local.get $arena
    i32.load
    i32.const 8
    i32.add
    i32.store
  )
  (func $allocate (param $size i32) (result i32)
    ;; Use current arena or fail if none
    global.get $current_arena
    local.get $size
    call $arena_alloc
  )

  ;; List operation functions
  (func $list_length (param $list i32) (result i32)
    ;; Load length from list header (offset 0)
    local.get $list
    i32.load
  )
  (func $list_get (param $list i32) (param $index i32) (result i32)
    (local $length i32)
    ;; Load length for bounds check
    local.get $list
    i32.load
    local.set $length
    
    ;; Bounds check
    local.get $index
    local.get $length
    i32.ge_u
    (if
      (then
        ;; Index out of bounds - trap
        unreachable
      )
    )
    
    ;; Calculate element address: list + 8 + (index * 4)
    local.get $list
    i32.const 8
    i32.add
    local.get $index
    i32.const 4
    i32.mul
    i32.add
    i32.load
  )
  (func $tail (param $list i32) (result i32)
    (local $length i32)
    (local $new_list i32)
    (local $new_length i32)
    
    ;; Load original length
    local.get $list
    i32.load
    local.set $length
    
    ;; Check if list is empty
    local.get $length
    i32.eqz
    (if
      (then
        ;; Return the same empty list
        local.get $list
        return
      )
    )
    
    ;; Calculate new length
    local.get $length
    i32.const 1
    i32.sub
    local.set $new_length
    
    ;; Allocate new list: 8 bytes header + (new_length * 4) bytes data
    local.get $new_length
    i32.const 4
    i32.mul
    i32.const 8
    i32.add
    call $allocate
    local.set $new_list
    
    ;; Write new length
    local.get $new_list
    local.get $new_length
    i32.store
    
    ;; Write new capacity (same as length)
    local.get $new_list
    i32.const 4
    i32.add
    local.get $new_length
    i32.store
    
    ;; Copy elements from original list (skip first element)
    local.get $new_list
    i32.const 8
    i32.add
    ;; destination
    local.get $list
    i32.const 12
    i32.add
    ;; source
    local.get $new_length
    i32.const 4
    i32.mul
    ;; size
    memory.copy
    
    local.get $new_list
  )

  ;; Array operation functions
  (func $array_get (param $array i32) (param $index i32) (result i32)
    ;; Calculate element address: array + (index * 4)
    local.get $array
    local.get $index
    i32.const 4
    i32.mul
    i32.add
    i32.load
  )
  (func $array_set (param $array i32) (param $index i32) (param $value i32)
    ;; Calculate element address: array + (index * 4)
    local.get $array
    local.get $index
    i32.const 4
    i32.mul
    i32.add
    local.get $value
    i32.store
  )

  ;; Functions
  (func $main (result i32)
    (local $list_tmp i32)
    (local $match_tmp i32)
    (local $tail_len i32)
    (local $tail_tmp i32)
    (local $n i32)
    (local $x i32)
    (local $y i32)
    (local $z i32)
    (local $a i32)
    (local $b i32)
    (local $c i32)
    (local $head i32)
    (local $tail i32)
    (local $rest i32)
    ;; Initialize default arena
    i32.const 32768
    call $arena_init
    global.set $current_arena

    i32.const 42

    ;; Reset default arena
    i32.const 32768
    call $arena_reset
  )

  ;; Export main
  (export "_start" (func $main))
)
