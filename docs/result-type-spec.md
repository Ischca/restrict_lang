# Result型の仕様

## 概要
Result型は成功または失敗を表現する型で、型安全なエラーハンドリングを提供します。

## 構文

### Result型の定義
```restrict
Result<T, E>  // T: 成功時の値の型, E: エラーの型
```

### Resultの生成
```restrict
// Ok: 成功の場合
val x: Result<Int32, String> = 42 Result::Ok
val y: Result<String, Int32> = "hello" Result::Ok

// Err: エラーの場合
val z: Result<Int32, String> = "not found" Result::Err
```

### パターンマッチング
```restrict
fun unwrap_or: (result: Result<Int32, String>, default: Int32) -> Int32 = {
    result match {
        Ok(n) => { n }
        Err(_) => { default }
    }
}
```

### 推奨される使用方法
```restrict
// 失敗する可能性のある関数
fun safe_divide: (a: Int32, b: Int32) -> Result<Int32, String> = {
    b == 0 then {
        "Division by zero" Result::Err
    } else {
        (a / b) Result::Ok
    }
}

// 使用例
fun main: () = {
    val result = (10, 2) safe_divide;
    result match {
        Ok(n) => { n println }
        Err(msg) => { msg println }
    }
}
```

## Option型との比較

| 型 | 値あり | 値なし/エラー | 用途 |
|---|---|---|---|
| `Option<T>` | `value Option::Some` | `() Option::None` | 値の存在/不在 |
| `Result<T, E>` | `value Result::Ok` | `error Result::Err` | 成功/失敗（エラー情報付き） |

## 標準ライブラリ関数

`std/result.rl` で提供される関数：

### 基本述語
```restrict
// Okかどうか判定
fun is_ok: <T, E> (result: Result<T, E>) -> Bool

// Errかどうか判定
fun is_err: <T, E> (result: Result<T, E>) -> Bool
```

### 値の取り出し
```restrict
// Ok値を取得、またはデフォルト値を返す
fun unwrap_or: <T, E> (result: Result<T, E>, default: T) -> T

// Err値を取得、またはデフォルト値を返す
fun unwrap_err_or: <T, E> (result: Result<T, E>, default: E) -> E
```

### 変換
```restrict
// Ok値を変換
fun map_ok: <T, U, E> (result: Result<T, E>, f: |T| -> U) -> Result<U, E>

// Err値を変換
fun map_err: <T, E, F> (result: Result<T, E>, f: |E| -> F) -> Result<T, F>

// Result返却関数をチェーン（flatMap）
fun and_then: <T, U, E> (result: Result<T, E>, f: |T| -> Result<U, E>) -> Result<U, E>

// ResultをOptionに変換（エラーを破棄）
fun ok: <T, E> (result: Result<T, E>) -> Option<T>

// ResultをOptionに変換（成功値を破棄）
fun err: <T, E> (result: Result<T, E>) -> Option<E>
```

## 実装詳細

### AST
- 値構築は `Call` / `Pipe` とqualified `VariantRef` に統一
- `Result::Ok` / `Result::Err` はbuiltin namespaceとして解決
- `Pattern::Ok(Box<Pattern>)` - Ok パターン
- `Pattern::Err(Box<Pattern>)` - Err パターン

### 型チェッカー
- `Result<T, E>` 型のサポート
- Ok/Err の型推論
- パターンマッチングでの網羅性チェック

### コード生成（WASM）
- タグ付きユニオンとして実装
- メモリレイアウト: `[tag: i32][value: i32]`（8バイト）
- discriminant: `1 = Ok`, `0 = Err`

```wat
;; value Result::Ok の生成
i32.const 8        ;; 8バイト確保
call $allocate
local.tee $ptr
i32.const 1        ;; tag = 1 (Ok)
i32.store
local.get $ptr
i32.const 4
i32.add
;; value を生成してstore
```

## 将来の拡張

### エラー伝播演算子 `?`
```restrict
// 将来実装予定
fun process: (data: String) -> Result<Int32, Error> = {
    val parsed = data parse?  // エラー時は早期リターン
    (parsed * 2) Result::Ok
}
```

### カスタムエラー型
```restrict
record ParseError {
    message: String
    line: Int
    column: Int
}

fun parse: (input: String) -> Result<AST, ParseError>
```
