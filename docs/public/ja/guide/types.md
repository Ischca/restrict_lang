# 型システム

Restrict Languageのv0.0.1で公開ドキュメントに載せる型は、仕様で現在サポートされる構文に合わせます。基本は静的型付け、アフィン型、OSV呼び出し、明示的なレコード型です。

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

## v0.0.1の範囲外

次の項目は設計または実装が進行中であり、公開ガイドでは現在の実行可能なRestrictコードとして扱いません。

- TATと時間スコープ付きリソース管理
- 借用スライスや参照型中心のAPI
- ユーザー定義の列挙型宣言、トレイト、関連型
- 旧来のRust風コレクションAPIやパス構文
- 文字列インポート、インポート別名、パッケージ単位の標準ライブラリ集約

## まとめ

v0.0.1の型システムでは、現在の構文で表せる基本型、ジェネリック型、レコード型、関数型を中心に書きます。例では常に`val`または`mut val`を使い、関数呼び出しは`value |> function`または`(a, b) function`のOSV形式に統一します。
