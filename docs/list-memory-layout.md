# リストのメモリレイアウト仕様

## 概要
Restrict Languageにおけるリストは、動的配列として実装されます。
Arenaアロケータを使用してメモリを確保します。

## メモリレイアウト

### リストヘッダー (8 bytes)
```
+--------+--------+
| length | capacity|  各4バイト（i32）
+--------+--------+
```

- **length**: 現在の要素数
- **capacity**: 確保済みの容量（将来の拡張用）

### リストデータ
```
+-------+-------+-------+-----+
| elem0 | elem1 | elem2 | ... |  各要素4バイト（現時点ではi32のみ）
+-------+-------+-------+-----+
```

## 実装例

### 空リスト `[]`
```
length: 0
capacity: 0
data: (なし)
```

### `[1, 2, 3]`
```
+---+---+
| 3 | 3 |  ヘッダー（length=3, capacity=3）
+---+---+
| 1 | 2 | 3 |  データ
+---+---+---+
```

## WASMでの実装

### リスト作成
```wasm
;; [1, 2, 3] の作成
i32.const 20        ;; ヘッダー(8) + データ(3*4) = 20 bytes
call $allocate      ;; メモリ確保
local.tee $list     ;; リストのアドレスを保存

;; length = 3
local.get $list
i32.const 3
i32.store

;; capacity = 3
local.get $list
i32.const 4
i32.add
i32.const 3
i32.store

;; data[0] = 1
local.get $list
i32.const 8
i32.add
i32.const 1
i32.store

;; data[1] = 2
local.get $list
i32.const 12
i32.add
i32.const 2
i32.store

;; data[2] = 3
local.get $list
i32.const 16
i32.add
i32.const 3
i32.store
```

### リスト操作関数

#### length取得
```wasm
(func $list_length (param $list i32) (result i32)
  local.get $list
  i32.load  ;; lengthフィールドを読み込む
)
```

#### 要素取得（インデックスアクセス）
```wasm
(func $list_get (param $list i32) (param $index i32) (result i32)
  ;; TODO: 境界チェック
  local.get $list
  i32.const 8
  i32.add              ;; データ部の開始位置
  local.get $index
  i32.const 4
  i32.mul              ;; インデックス * 4
  i32.add
  i32.load             ;; 要素を読み込む
)
```

## 型の拡張

将来的には、要素の型情報をヘッダーに含めることで、
任意の型のリストをサポートできます：

```
+--------+--------+--------+
| length | capacity| type   |  12 bytes
+--------+--------+--------+
```

- type: 0=i32, 1=f64, 2=string, 3=record, etc.