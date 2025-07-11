# Arena Allocator 仕様書

## 概要
Arenaアロケータは、Restrict Languageのメモリ管理の中核となるコンポーネントです。
`with Arena`ブロック内で確保されたメモリは、ブロック終了時に一括解放されます。

## 設計方針

1. **シンプルさ**: バンプアロケータ方式で高速割り当て
2. **安全性**: ブロックスコープでの自動解放
3. **WASM互換**: WASMのlinear memoryと親和性の高い設計

## WASM実装詳細

### メモリレイアウト
```
+------------------+
| Static Data      | 0x0000 - 0x0FFF (4KB)
| - String literals|
| - Constants      |
+------------------+
| Stack            | 0x1000 - 0x7FFF (28KB) 
| - Local vars     |
| - Call frames    |
+------------------+
| Arena 1          | 0x8000 - 0xFFFF (32KB)
| - Dynamic allocs |
+------------------+
| Arena 2          | 0x10000 - 0x17FFF (32KB)
| - Nested arena   |
+------------------+
| ...              |
+------------------+
```

### Arena構造体（WASMメモリ上）
```
Arena Header (8 bytes):
+--------+--------+
| start  | current|  各4バイト（i32）
+--------+--------+

start: Arenaの開始アドレス
current: 次の割り当て位置
```

### 基本操作

#### 1. Arena初期化
```wasm
(func $arena_init (param $start i32) (result i32)
  ;; Arena headerの開始位置を返す
  local.get $start
  ;; startフィールドを設定
  local.get $start
  local.get $start
  i32.store
  ;; currentフィールドを設定（start + 8）
  local.get $start
  i32.const 8
  i32.add
  local.get $start
  i32.const 8
  i32.add
  i32.store
  ;; Arena headerのアドレスを返す
  local.get $start
)
```

#### 2. メモリ割り当て
```wasm
(func $arena_alloc (param $arena i32) (param $size i32) (result i32)
  (local $current i32)
  (local $new_current i32)
  
  ;; currentフィールドを読み込む
  local.get $arena
  i32.const 4
  i32.add
  i32.load
  local.set $current
  
  ;; アライメント（4バイト境界）
  local.get $size
  i32.const 3
  i32.add
  i32.const -4
  i32.and
  local.set $size
  
  ;; 新しいcurrent位置を計算
  local.get $current
  local.get $size
  i32.add
  local.set $new_current
  
  ;; TODO: オーバーフローチェック
  
  ;; currentフィールドを更新
  local.get $arena
  i32.const 4
  i32.add
  local.get $new_current
  i32.store
  
  ;; 割り当てたメモリのアドレスを返す
  local.get $current
)
```

#### 3. Arena解放（リセット）
```wasm
(func $arena_reset (param $arena i32)
  ;; currentをstartに戻す
  local.get $arena
  i32.const 4
  i32.add
  local.get $arena
  i32.load
  i32.const 8
  i32.add
  i32.store
)
```

## 言語統合

### コンテキストとしてのArena
```ocaml
context Arena {
  // 内部的にArena headerへのポインタを保持
}
```

### with文での使用
```ocaml
with Arena {
  val list = [1, 2, 3, 4, 5];  // arena_allocを呼ぶ
  // 処理...
}  // arena_resetが自動的に呼ばれる
```

### ネストされたArena
```ocaml
with Arena {  // Arena 1
  val outer = [1, 2, 3];
  
  with Arena {  // Arena 2（別領域）
    val inner = [4, 5, 6];
  }  // Arena 2解放
  
  // outerはまだ有効
}  // Arena 1解放
```

## 実装計画

1. **Phase 1**: 基本的なArenaアロケータ
   - arena_init, arena_alloc, arena_reset
   - 固定サイズArena（32KB）

2. **Phase 2**: 言語統合
   - Arenaコンテキストの実装
   - with文でのArena自動管理

3. **Phase 3**: リスト実装
   - リストのメモリレイアウト設計
   - Arena上でのリスト操作

4. **Phase 4**: 最適化
   - 複数Arena管理
   - メモリ不足時の処理