# 構文リファレンス

このガイドは、v0.0.1 の公開構文と、意図的に範囲外としている将来構文を区別して説明します。

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

パターンには、ワイルドカード、変数束縛、リテラル、`Some`、`None`、リスト、レコード、修飾されたenumバリアントが使えます。

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

## ユーザー定義enum

現在のcompilerでは、閉じたユーザー定義enumを宣言できます。

```restrict
enum ParseError {
    Empty
    Message(String)
}

fun empty_error: () -> ParseError = {
    () ParseError::Empty
}

fun message_error: (message: String) -> ParseError = {
    message |> ParseError::Message
}
```

バリアント名は常に`ParseError::Empty`のように`型名::バリアント名`で修飾します。payloadなしのコンストラクタはunitを受け取り、payloadありのコンストラクタは宣言された型の値を1つだけ受け取ります。どちらも直接のOSV呼び出し対象であり、`ParseError::Message("error")`のような従来順の呼び出しは使えません。

現在のenum宣言は非ジェネリックかつ非再帰で、各バリアントが持てるpayloadは0個または1個です。複数の値をまとめたい場合はrecordを1つのpayloadにします。

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

## スコープ動詞節

最後に残った仮引数が関数型である動詞は、その関数をスコープとして
開けます。

```restrict
val shifted = values map {
    it + 1
}

val total = (shifted, 0) fold { |sum, value|
    sum + value
}
```

ヘッダーのない形式は文脈的な`it`束縛を1つ導入します。明示スコープの
ヘッダーはラムダのバインダーを再利用します。完成したスコープ節は、
後続の節やパイプより先に評価されます。

コールバック形式、コレクション動作、型推論、アフィンなキャプチャは
[高階関数とコレクション変換](../advanced/higher-order.md)を参照してください。

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
val maybe_score: Option<Int32> = 100 Option::Some
val no_score: Option<Int32> = () Option::None

val success: Result<Int32, String> = 42 Result::Ok
val failure: Result<Int32, String> = "error" Result::Err
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

pub enum PublicError {
    Missing
    Invalid(String)
}
```

v0.0.1 では、export されたレコードや generic 関数はソースレベルのモジュールメタデータです。v0.0.1の`pub enum`もRestrictソースモジュール間だけの公開で、直接のhost-visible WebAssembly enum ABIは提供しません。

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

1. フィールドアクセス、修飾されたバリアント名、グループ化された直接OSV呼び出し、スコープ動詞節: `.field`、`.clone`、`Type::Variant`、`freeze`、`(value) f`、`() f`、`values map { ... }`
2. 単項演算子: `!`、`-`
3. 乗除余: `*`、`/`、`%`
4. 加減: `+`、`-`
5. 比較: `<`、`<=`、`>`、`>=`
6. 等価: `==`、`!=`
7. 論理 AND: `&&`
8. 論理 OR: `||`
9. パイプ: `|>`

## 現在のenum境界

v0.0.1 compilerは上記の閉じたユーザー定義enumをサポートします。ジェネリックenum、再帰enum、1バリアントに複数の直接payloadを持たせる構文、host enum ABIは将来の設計対象です。`Result<T, E>`のエラー伝播は`match`で明示し、`?`演算子はまだ使えません。

## v0.0.1 の current example ではない構文

次の項目は予約済み、実験中、または将来の設計対象です。公開ドキュメントで current Restrict のコード例として扱う場合は、実装状況を確認してください。

- temporal affine types と lifetime scope
- TAT cleanup
- associated type、generic form、default method、generic/conditional/enum adoption
- derive、属性、dynamic dispatch
- ループラベルと範囲パターン
- パッケージ単位の標準ライブラリ集約 import
- メソッド呼び出し形式の通常関数
- 可変パイプ演算子

## まとめ

v0.0.1 の Restrict は、`val`、`mut val`、OSV 呼び出し、明示的な `fun name: (...) -> Type = { ... }` 構文、dotted source import を中心にしています。迷った場合は、必ず `/LANGUAGE_SPECIFICATION.md` を優先してください。
