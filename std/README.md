# Restrict Language 標準ライブラリ

このディレクトリには、Restrict Language の標準ライブラリが含まれています。

## モジュール構成

### `prelude.rl`
自動的にインポートされる基本的な関数とユーティリティ。

### `math.rl`
数学的な計算に関する関数群。
- `abs(x)` - 絶対値
- `max(a, b)` - 最大値
- `min(a, b)` - 最小値
- `pow(base, exp)` - 累乗
- `factorial(n)` - 階乗

### `string.rl`
文字列操作に関する関数群。
- `string_length(s)` - 文字列長
- `string_is_empty(s)` - 空文字列判定
- `string_concat(a, b)` - 文字列連結
- `string_to_int(s)` - 文字列から整数への変換
- `int_to_string(n)` - 整数から文字列への変換

### `list.rl`
リスト操作に関する関数群。
- `list_is_empty(list)` - 空リスト判定
- `list_head(list)` - 先頭要素取得
- `list_tail(list)` - 末尾リスト取得
- `list_reverse(list)` - リスト反転
- `list_append(list, item)` - 要素追加
- `list_concat(a, b)` - リスト連結
- `list_count(list)` - 要素数取得
- `list_filter(list, predicate)` - フィルタリング
- `list_map(list, f)` - マッピング
- `list_fold_left(list, acc, f)` - 左畳み込み

### `option.rl`
Option型操作に関する関数群。
- `option_is_some(opt)` - 値を持つか判定
- `option_is_none(opt)` - 空か判定
- `option_unwrap_or(opt, default)` - デフォルト値付き取得
- `option_map(opt, f)` - マッピング
- `option_and_then(opt, f)` - モナディック操作

### `io.rl`
入出力に関する関数群。
- `print(s)` - 文字列出力
- `print_int(n)` - 整数出力
- `debug_print(value)` - デバッグ出力
- `eprint(s)` - エラー出力
- `read_line()` - 一行読み取り（未実装）

## 使用例

```restrict
import std.math.{abs, max}
import std.list.{list_map, list_filter}

fun main() {
    val numbers = [1, -2, 3, -4, 5]
    val positives = list_filter(numbers, |x| x > 0)
    val absolutes = list_map(numbers, abs)
    
    print_int(max(10, 20))  // 20
}
```

## 実装状況

- ✅ 基本的な関数シグネチャ定義
- ✅ 数学関数の実装
- ✅ リスト操作の実装
- ✅ Option型操作の実装
- ⚠️ 文字列操作（一部未実装）
- ⚠️ IO操作（一部未実装）

## 今後の予定

1. 文字列操作のWebAssembly実装
2. ファイルIO機能の追加
3. 正規表現サポート
4. 日付・時刻操作
5. JSON解析機能
6. ネットワーク機能（HTTP）