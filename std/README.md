# Restrict Language 標準ライブラリ

このディレクトリには、Restrict Language の標準ライブラリが含まれています。

## モジュール構成

### `prelude.rl`
自動的にインポートされる基本的な関数とユーティリティ。

### `math.rl`
数学的な計算に関する関数群。
- `x |> abs` - 絶対値
- `(a, b) max` - 最大値
- `(a, b) min` - 最小値
- `(base, exp) pow` - 累乗
- `n |> factorial` - 階乗

### `string.rl`
現在の文字列 surface は、文字列リテラル、`+` による結合、`==` / `!=`
による比較です。長さ取得、パース、フォーマット、trim、split などの
ヘルパーは v0.0.1 の標準ライブラリ surface には含まれていません。

### `list.rl`
リスト操作に関する関数群。
- `list |> list_is_empty` - 空リスト判定
- `list |> list_head` - 先頭要素取得
- `list |> list_tail` - 末尾リスト取得
- `list |> list_reverse` - リスト反転
- `(item, list) list_prepend` - 先頭追加
- `(list, item) list_append` - 要素追加
- `(a, b) list_concat` - リスト連結
- `list |> list_count` - 要素数取得
- `list |> list_length` - 要素数取得
- `(list, index) list_get` - インデックス取得

`map`、`filter`、`fold` は `prelude.rl` の compiler-registered generic
container builtin として扱います。

### `option.rl`
Option型操作に関する関数群。
- `opt |> option_is_some` - 値を持つか判定
- `opt |> option_is_none` - 空か判定
- `(opt, default) option_unwrap_or` - デフォルト値付き取得

### `io.rl`
入出力に関する関数群。
- `s |> print` - 文字列出力
- `n |> print_int` - 整数出力
- `f |> print_float` - 浮動小数出力
- `s |> eprint` - エラー出力
- `s |> eprintln` - 改行付きエラー出力

標準入力やファイルI/Oは current v0.0.1 surface には含まれていません。

## 使用例

```restrict
fun main: () -> Int32 = {
    val numbers = [1, -2, 3, -4, 5]
    val positives = (numbers, |x| x > 0) filter
    val absolutes = (positives, abs) map

    (10, 20) max |> print_int
    absolutes |> list_count
}
```

現在の v0.0.1 では、これらの標準関数はコンパイラに登録された組み込み
surface として利用します。標準ライブラリの集約モジュール import や import
alias は、v0.0.1 の標準ライブラリ surface には含まれていません。
