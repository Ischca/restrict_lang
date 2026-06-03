# 標準ライブラリリファレンス

このページは、v0.0.1 の current standard-library surface を説明します。現在の標準関数は主にコンパイラへ登録された組み込み surface として提供され、`std/*.rl` は読者とテスト向けの参照インデックスです。

## import について

v0.0.1 では、標準ライブラリを集約モジュールとして読み込む構文は current surface ではありません。ソースモジュールの import は dotted path 形式だけを扱いますが、標準ライブラリの package-level aggregator は今後のモジュール設計対象です。

## prelude.rl

自動的に利用できる基本関数です。

```text
identity: <T>(T) -> T
map: compiler-registered generic container mapping builtin
filter: compiler-registered generic container filtering builtin
fold: compiler-registered generic List reduction builtin
not: (Boolean) -> Boolean
and: (Boolean, Boolean) -> Boolean
or: (Boolean, Boolean) -> Boolean
assert: (Boolean, String) -> ()
panic: (String) -> ()
```

```restrict
fun prelude_example: () -> Boolean = {
    val value = 42 |> identity
    val inverted = false |> not
    val both = (true, inverted) and
    mut val result = (both, value == 42) or

    (result, "prelude example should pass") assert
    result
}
```

## io.rl

現在の I/O surface はコンソール出力に限定されています。

```text
println: (String) -> ()
print: (String) -> ()
print_int: (Int32) -> ()
print_float: (Float64) -> ()
eprint: (String) -> ()
eprintln: (String) -> ()
```

```restrict
fun io_example: () -> () = {
    "Hello" |> print
    "World" |> println
    42 |> print_int
    3.14 |> print_float
    "error" |> eprintln
}
```

標準入力、ファイル読み書き、ディレクトリ操作は current standard-library surface には含まれていません。

## string.rl

現在の文字列 surface は、文字列結合と内容比較です。

```text
left + right: concatenate two String values
left == right: compare String contents
left != right: compare String contents and negate the result
```

```restrict
fun string_example: () -> Boolean = {
    val joined = "Hello, " + "World"
    val matches = joined == "Hello, World"
    matches
}
```

長さ取得、パース、フォーマット、trim、split などのヘルパーは current surface には含まれていません。

## math.rl

```text
abs: (Int32) -> Int32
max: (Int32, Int32) -> Int32
min: (Int32, Int32) -> Int32
pow: (Int32, Int32) -> Int32
factorial: (Int32) -> Int32
abs_f: (Float64) -> Float64
max_f: (Float64, Float64) -> Float64
min_f: (Float64, Float64) -> Float64
```

```restrict
fun math_example: () -> Int32 = {
    val a = -5 |> abs
    val b = (10, 20) max
    val c = (3, 7) min
    val d = (2, 3) pow
    val e = 4 |> factorial

    a + b + c + d + e
}
```

```restrict
fun float_math_example: () -> Float64 = {
    val a = -3.14 |> abs_f
    val b = (1.5, 2.7) max_f
    val c = (0.5, 1.0) min_f

    a + b + c
}
```

三角関数、丸め、対数、乱数は current surface には含まれていません。

## list.rl

```text
list_is_empty: <T>(List<T>) -> Boolean
list_head: <T>(List<T>) -> Option<T>
list_tail: <T>(List<T>) -> Option<List<T>>
list_reverse: <T>(List<T>) -> List<T>
list_prepend: <T>(T, List<T>) -> List<T>
list_append: <T>(List<T>, T) -> List<T>
list_concat: <T>(List<T>, List<T>) -> List<T>
list_count: <T>(List<T>) -> Int32
list_length: <T>(List<T>) -> Int32
list_get: <T>(List<T>, Int32) -> T
```

```restrict
fun list_example: () -> Int32 = {
    mut val numbers = [1, 2, 3, 4]

    val count = numbers |> list_count
    val length = numbers |> list_length
    val first = (numbers, 0) list_get
    val extended = (numbers, 5) list_append
    val extended_count = extended |> list_count

    count + length + first + extended_count
}
```

```restrict
fun list_composition_example: () -> Int32 = {
    val extended = ([1, 2, 3, 4], 5) list_append
    val combined = ([1, 2, 3, 4], [6, 7, 8]) list_concat
    val first = (0, [1, 2, 3, 4]) list_prepend

    val a = extended |> list_count
    val b = combined |> list_count
    val c = first |> list_count

    a + b + c
}
```

専用の list map/filter/fold helper は current list module surface には含まれていません。prelude の compiler-registered generic container builtins として扱われます。

## option.rl

```text
option_is_some: <T>(Option<T>) -> Boolean
option_is_none: <T>(Option<T>) -> Boolean
option_unwrap_or: <T>(Option<T>, T) -> T
```

```restrict
fun option_example: () -> Int32 = {
    mut val maybe = Some(42)

    val has_value = maybe |> option_is_some
    val is_empty = maybe |> option_is_none
    val value = (maybe, 0) option_unwrap_or

    value
}
```

`Some(value)` と `None` は source-level constructor syntax として扱います。option map、flatten、and-then、zip などの高階 helper は current surface には含まれていません。

## 組み合わせ例

```restrict
fun standard_library_flow: () -> Boolean = {
    val abs_result = -42 |> abs
    val max_result = (abs_result, 50) max

    mut val numbers = [1, 2, 3, 4, 5]
    val list_size = numbers |> list_count
    mut val maybe_first = numbers |> list_head

    mut val first = (maybe_first, 0) option_unwrap_or
    val has_value = maybe_first |> option_is_some

    val above_threshold = max_result > 40
    mut val result = (has_value, above_threshold) and

    first |> print_int
    list_size |> print_int
    (result, "standard library flow should pass") assert

    result
}
```

## v0.0.1 の current surface ではない標準 API

次の領域は、今後の標準ライブラリまたはランタイム設計で扱う予定です。このページでは current Restrict のコード例としては示しません。

- 標準入力とファイルシステム
- 時刻、乱数、ネットワーク、プロセス管理
- 反復子 trait とカスタム iterator
- Display、Debug、Hash などの trait 実装
- macro ベースのフォーマット API
- try-style early return operator

標準ライブラリの例を追加する場合は、`std/*.rl` の参照インデックスと compiler/runtime support が一致していることを先に確認してください。
