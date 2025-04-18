(module
  (type (;0;) (func (param f64) (result f64)))
  (import "env" "__linear_memory" (memory (;0;) 0))
  (func (;0;) (type 0) (param f64) (result f64)
    (local f64 i32 i32 i32 i32 f64 f64 f64 f64 f64 f64 f64 f64 f64)
    f64.const 0x1p+1 (;=2;)
    local.set 1
    local.get 0
    local.get 1
    f64.ge
    local.set 2
    local.get 2
    i32.eqz
    local.set 3
    i32.const 1
    local.set 4
    local.get 3
    local.get 4
    i32.and
    local.set 5
    local.get 0
    local.set 6
    block  ;; label = @1
      local.get 5
      br_if 0 (;@1;)
      f64.const -0x1p+0 (;=-1;)
      local.set 7
      local.get 0
      local.get 7
      f64.add
      local.set 8
      local.get 8
      call 0
      local.set 9
      f64.const -0x1p+1 (;=-2;)
      local.set 10
      local.get 0
      local.get 10
      f64.add
      local.set 11
      local.get 11
      call 0
      local.set 12
      local.get 9
      local.get 12
      f64.add
      local.set 13
      local.get 13
      local.set 6
    end
    local.get 6
    local.set 14
    local.get 14
    return))
