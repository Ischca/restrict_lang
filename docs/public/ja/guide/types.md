# 型システム

Restrict Languageのv0.0.1で公開する型は、仕様で現在サポートされる構文に合わせて説明します。基本は静的型付け、アフィン型、OSV呼び出し、明示的なレコード型です。

## 基本型

現在の基本型名は大文字始まりです。

```restrict
val count: Int32 = 42
val total: Int64 = 1_000_000
val ratio: Float64 = 0.75
val title: String = "release"
val marker: Char = 'R'
val ready: Boolean = true
val unit_value: () = ()
```

よく使う型は次の通りです。

- `Int32`, `Int64`
- `Float64`
- `String`, `Char`
- `Boolean`
- `()`（ユニット型）

## アフィン型

Restrictの値は、基本的に最大1回まで使用できます。値を関数に渡すと、その値の所有権も渡されます。

```restrict
fun consume_title: (title: String) -> String = {
    title
}

fun main: () -> String = {
    val title = "Restrict"
    title |> consume_title
}
```

同じ値を何度も読む設計ではなく、必要な値を明示的に渡していく設計にします。単純な数値や真偽値のようなコピー可能な基本型は、実装側のコピー意味論に従います。

## 可変束縛

可変束縛は`mut val`です。語順は固定です。

```restrict
fun main: () -> Int32 = {
    mut val counter = 0
    counter = counter + 1
    counter
}
```

## コレクション型

リストと固定長配列はジェネリック型で表します。範囲はv0.0.1では`Range<Int32>`だけを公開します。

```restrict
fun total_first_two: () -> Int32 = {
    val scores: List<Int32> = [10, 20, 30]
    scores match {
        [first, second] => { first + second }
        [first | rest] => { first }
        [] => { 0 }
    }
}
```

```restrict
val empty_scores: List<Int32> = []
val range: Range<Int32> = [1..5]
```

型としては`List<T>`、`Array<T, N>`、`Range<Int32>`を使います。v0.0.1の範囲リテラルはInt32の開始値と終了値だけを扱います。固定長配列の詳細な標準APIは、v0.0.1では実装と標準ライブラリの進行に合わせて扱います。

## OptionとResult

オプショナルな値は`Option<T>`、成功または失敗は`Result<T, E>`で表します。

```restrict
fun value_or_zero: (value: Option<Int32>) -> Int32 = {
    value match {
        Some(number) => { number }
        None => { 0 }
    }
}
```

```restrict
fun result_or_zero: (result: Result<Int32, String>) -> Int32 = {
    result match {
        Ok(value) => { value }
        Err(message) => { 0 }
    }
}
```

## ユーザー定義enum

現在のcompilerでは、閉じたタグ付き和としてユーザー定義enumを宣言できます。宣言は非ジェネリックかつ非再帰で、各バリアントのpayloadは0個または1個です。

```restrict
enum DecodeError {
    Empty
    Invalid(String)
}

fun reject: (message: String) -> Result<Int32, DecodeError> = {
    (message |> DecodeError::Invalid) Result::Err
}

fun error_code: (error: DecodeError) -> Int32 = {
    error match {
        DecodeError::Empty => { 1 }
        DecodeError::Invalid(message) => { 2 }
    }
}
```

バリアントは`DecodeError::Invalid`のように必ずenum名で修飾します。payloadなしのバリアントは`() DecodeError::Empty`、payloadありのバリアントは`message |> DecodeError::Invalid`のようにOSV語順で構築します。`match`はenumの全バリアントを網羅する必要があります。

enum値がCopyになるのは、すべてのpayload型がCopyの場合だけです。アフィンなpayloadを構築するとその値は移動し、enumを`match`するとscrutineeは通常のアフィン規則に従って1回消費されます。

この初期範囲ではenum値に`==`と`!=`は定義しません。内部の割り当てアドレスを誤って比較しないようcompilerがenumの等価比較を拒否するため、修飾されたバリアントを`match`してください。

`pub enum`は別のRestrictソースモジュールからimportできる型名前空間を公開しますが、直接のhost-visible WebAssembly enum ABIは提供しません。`Result<T, DecodeError>`のエラー伝播は明示的な`match`で行い、`?`演算子はまだありません。

## レコード型

関連する値は`record`でまとめます。フィールド定義とレコードリテラルのどちらも、フィールド名の後に`:`を置きます。

```restrict
record User {
    name: String
    age: Int32
    active: Boolean
}

fun user_name: (user: User) -> String = {
    user.name
}

fun main: () -> String = {
    val user = User { name: "Alice", age: 30, active: true }
    user |> user_name
}
```

## 関数型とジェネリクス

関数型は`A -> B`で表します。関数宣言は`fun name: (...) -> Type = { ... }`です。

```restrict
fun identity: <T>(value: T) -> T = {
    value
}

fun apply_once: <T, U>(value: T, transform: T -> U) -> U = {
    value |> transform
}
```

複数引数の呼び出しはOSVのタプル形式です。

```restrict
fun add: (left: Int32, right: Int32) -> Int32 = {
    left + right
}

fun main: () -> Int32 = {
    (20, 22) add
}
```

## 型推論

型注釈は必要な場所にだけ書けます。公開APIや曖昧になりやすい空リストでは、明示すると読みやすくなります。

```restrict
fun main: () -> Int32 = {
    val answer = 42
    val numbers: List<Int32> = []
    numbers match {
        [] => { answer }
        [first | rest] => { first }
    }
}
```

## Form と静的ポリモーフィズム

現在のcompilerでは、method-only `form`、具体的な非ジェネリックrecordの
`takes`、`<T of Form>`境界を利用できます。呼び出しは静的に選択され、具体型
ごとにmonomorphizeされます。

```restrict
form Labelled {
    fun label: (self: Self) -> String
}

record Badge {
    text: String
}

Badge takes Labelled {
    fun label: (self: Badge) -> String = {
        self.text
    }
}

fun read_label: <T of Labelled>(value: T) -> String = {
    value |> label
}
```

詳細とDisplay出力については[Form と静的ポリモーフィズム](forms.md)を参照してください。

## 現在の範囲外

次の項目は設計または実装が進行中であり、公開ガイドでは現在の実行可能なRestrictコードとして扱いません。

- TATと時間スコープ付きリソース管理
- 借用スライスや参照型中心のAPI
- ジェネリックenum、再帰enum、複数の直接payloadを持つバリアント
- ユーザー定義enumのhost-visible WebAssembly ABIと`?`演算子
- associated type、generic form、default method、generic/conditional/enum
  adoption、dynamic dispatch
- 旧来のRust風コレクションAPIやパス構文
- 文字列インポート、インポート別名、パッケージ単位の標準ライブラリ集約

ユーザー定義enumはv0.0.1の公開範囲です。前節で説明した閉じた範囲を利用できます。

## まとめ

現在の型システムでは、基本型、組み込みジェネリック型、レコード型、関数型に加え、制約されたユーザー定義enumと静的formを利用できます。例では常に`val`または`mut val`を使い、関数呼び出しとenum構築は`value |> function`、`(a, b) function`、`() Type::Variant`などのOSV形式に統一します。
