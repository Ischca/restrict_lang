# 構文リファレンス

このガイドは、v0.0.1 の current surface として扱える Restrict Language 構文を中心に説明します。将来予定の構文は、最後のセクションに分けています。

## コメント

```restrict
// 単一行コメント

/*
   複数行コメント
*/
```

## 識別子とキーワード

識別子は文字またはアンダースコアで始まり、その後に文字、数字、またはアンダースコアが続きます。

```restrict
val valid_name = 1
val _private = 2
val camelCase = 3
val snake_case = 4
val number123 = 5
```

予約キーワードは次のとおりです。予約済みであっても、関連するすべての構文が v0.0.1 で実装済みとは限りません。

```text
fun
val
mut
record
context
enum
match
then
else
while
temporal
within
where
clone
freeze
pub
import
export
as
fatal
true
false
Some
None
with
lifetime
await
spawn
```

## リテラル

```restrict
val decimal = 42
val hex = 0xFF
val with_underscores = 1_000_000

val float_value = 3.14
val scientific = 2.5e-10

val simple = "Hello, World!"
val escaped = "Line 1\nLine 2\tTabbed"

val letter = 'a'
val newline = '\n'

val yes = true
val no = false
val unit_value = ()
```

バイナリ・8進数リテラル、生文字列、複数行文字列は、このページでは v0.0.1 の current example として扱いません。

## 変数と束縛

```restrict
val x = 42
val y: Int32 = 42
val pi: Float64 = 3.14

mut val counter = 0
counter = counter + 1
```

不変束縛は `val`、複数回の使用や再代入が必要な束縛は `mut val` を使います。

## 基本式

```restrict
val sum = 1 + 2
val difference = 5 - 3
val product = 4 * 3
val quotient = 10 / 2
val remainder = 7 % 3

val equal = x == y
val not_equal = x != y
val less = x < y
val greater = x > y
val less_eq = x <= y
val greater_eq = x >= y

val and_result = true && false
val or_result = true || false
val not_result = !true
```

べき乗、ビット演算、シフト演算は、v0.0.1 の current examples からは外しています。

## 条件式

Restrict は `then` と `else` を使います。

```restrict
val label = score >= 80 then {
    "pass"
} else {
    "retry"
}

val greeting = hour < 12 then {
    "おはよう"
} else {
    hour < 18 then {
        "こんにちは"
    } else {
        "こんばんは"
    }
}
```

## match 式

`match` は値の後ろに置きます。

```restrict
val description = number match {
    0 => { "ゼロ" }
    1 => { "一" }
    _ => { "その他" }
}

val unwrapped = maybe_value match {
    Some(value) => { value }
    None => { 0 }
}
```

パターンには、ワイルドカード、変数束縛、リテラル、`Some`、`None`、リスト、レコードが使えます。

```restrict
val first_or_zero = values match {
    [] => { 0 }
    [head | tail] => { head }
}

val label = point match {
    Point { x: 0, y: 0 } => { "origin" }
    Point { x, y } => { "point" }
}
```

ガード付きパターン、範囲パターン、テスト専用属性は、v0.0.1 の current examples としては扱いません。

## リストとレコード

```restrict
val numbers = [1, 2, 3]
val empty_numbers: List<Int32> = []

record Point {
    x: Int32
    y: Int32
}

val origin = Point { x: 0, y: 0 }
```

レコード定義とレコードリテラルのフィールドは `:` を使います。

## 関数

```restrict
fun add: (x: Int32, y: Int32) -> Int32 = {
    x + y
}

fun double: (x: Int32) -> Int32 = {
    x * 2
}

fun answer: () -> Int32 = {
    42
}

fun identity: <T>(value: T) -> T = {
    value
}
```

関数呼び出しは OSV 構文だけです。引数が先、関数名が後です。

```restrict
val total = (10, 20) add
val doubled = 21 |> double
val known = () answer
```

## ラムダ式と関数型

```restrict
val add_one = |x: Int32| x + 1
val multiply = |x: Int32, y: Int32| x * y

val transformer: Int32 -> Int32 = |x: Int32| x * 2
val reducer: (Int32, Int32) -> Int32 = |left: Int32, right: Int32| left + right
```

## 型

### 基本型

```text
Int32
Int64
Float64
String
Char
Boolean
()
```

### ジェネリック型

```restrict
val maybe_score: Option<Int32> = Some(100)
val no_score: Option<Int32> = None

val success: Result<Int32, String> = Ok(42)
val failure: Result<Int32, String> = Err("error")
```

`List<T>`、`Array<T, N>`、`Option<T>`、`Result<T, E>` は仕様上のジェネリック型です。`Range<Int32>` は v0.0.1 で Int32 の開始値と終了値だけを扱う組み込みコレクション型です。

## インポート

v0.0.1 の import は、ソースモジュールの dotted path だけを扱います。

```restrict
import release.policy.{score}
import release.policy.*
import release.policy
```

文字列パス、別名付き import、再 export、標準ライブラリ集約 import は、今後のモジュール設計で扱います。

## pub 宣言

```restrict
pub fun public_score: (input: Int32) -> Int32 = {
    input
}

pub record PublicPoint {
    x: Int32
    y: Int32
}
```

v0.0.1 では、export されたレコードや generic 関数はソースレベルのモジュールメタデータです。直接の host-visible WebAssembly ABI としては扱いません。

## context と with

```restrict
context Request {
    user: String
    trace_id: String
}

with Request { user: "alice", trace_id: "req-1" } {
    "request accepted" |> println
}
```

関数宣言に context を注入する注釈構文は、v0.0.1 の current example ではありません。

## clone と freeze

```restrict
record Settings {
    retries: Int32
    timeout: Int32
}

val base = Settings { retries: 3, timeout: 10 } freeze
val strict = base.clone { timeout: 3 }
```

## 演算子の優先順位

1. フィールドアクセス: `.field`、`.clone`、`freeze`
2. 単項演算子: `!`、`-`
3. 乗除余: `*`、`/`、`%`
4. 加減: `+`、`-`
5. 比較: `<`、`<=`、`>`、`>=`
6. 等価: `==`、`!=`
7. 論理 AND: `&&`
8. 論理 OR: `||`
9. パイプ: `|>`
10. OSV 関数呼び出し

## v0.0.1 の current example ではない構文

次の項目は予約済み、実験中、または将来の設計対象です。公開ドキュメントで current Restrict のコード例として扱う場合は、実装状況を確認してください。

- temporal affine types と lifetime scope
- TAT cleanup
- trait、impl、derive、属性
- ループラベルと範囲パターン
- パッケージ単位の標準ライブラリ集約 import
- メソッド呼び出し形式の通常関数
- 可変パイプ演算子

## まとめ

v0.0.1 の Restrict は、`val`、`mut val`、OSV 呼び出し、明示的な `fun name: (...) -> Type = { ... }` 構文、dotted source import を中心にしています。迷った場合は、必ず `/LANGUAGE_SPECIFICATION.md` を優先してください。
