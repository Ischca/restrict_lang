# Restrict Language 標準ライブラリ

このディレクトリには、コンパイラに組み込まれている標準ライブラリモジュール (`std.*`) のソースが含まれています。
`import std.<module>.*`、`import std.<module>.{...}`、`import std.<module>` で任意のプロジェクトからインポートできます。

## モジュール構成

### `std.prelude` (`prelude.rl`)
コンパイラ組み込みの `identity`、`map`、`filter`、`fold`、`not`、`and`、`or`、`assert`、`panic` に加え、
以下の純粋 Restrict ヘルパーを提供します。
- `value |> identity_val`
- `(left, right) bool_xor` / `(cond, consequent) bool_implies` / `flag |> bool_to_int`
- `(left, right) eq_int` / `(left, right) ne_int` / `(left, right) cmp_int` / `cmp_int64` / `cmp_float`

### `std.math` (`math.rl`)
組み込みの `abs`、`max`、`min`、`pow`、`factorial`、`abs_f`、`max_f`、`min_f` に加え、
以下のヘルパーを提供します。
- `(value, low, high) clamp` / `value |> signum` / `value |> is_even` / `value |> is_odd`
- `(a, b) gcd` / `(a, b) lcm`
- `abs_i64` / `min_i64` / `max_i64` / `clamp_i64` / `signum_i64`
- `clamp_f` / `lerp_f` / `approx_eq_f`

### `std.option` (`option.rl`)
組み込みの `option_is_some`、`option_is_none`、`option_unwrap_or` に加え、以下を提供します。
- `(opt, fallback) option_or` / `(opt, fallback_fn) option_unwrap_or_else`
- `(opt, f) option_and_then` / `(opt, error) option_ok_or`
- `nested |> option_flatten` / `(left, right, combine) option_zip_with`

### `std.result` (`result.rl`)
- `res |> result_is_ok` / `res |> result_is_err`
- `(res, fallback) result_unwrap_or` / `(res, recover) result_unwrap_or_else`
- `(res, f) result_map` / `(res, f) result_map_err` / `(res, f) result_and_then`
- `res |> result_ok` / `res |> result_err`

### `std.list` (`list.rl`)
組み込みのリスト演算に加え、以下を提供します。
- `IndexedInt` レコード
- `(items, pred) list_any` / `(items, pred) list_all` / `(items, pred) list_find`
- `(items, count) list_take` / `(items, count) list_drop` / `(start, end) list_range`
- `items |> list_sum` / `list_sum_i64` / `list_sum_f` / `list_product`
- `(items, start) list_enumerate_from` / `(left, right, combine) list_zip_with`

### `std.string` (`string.rl`)
- `s |> string_is_empty` / `s |> string_non_empty`
- `(s, count) string_repeat` / `(s, pad, count) string_pad_left`
- `(parts, sep) string_join_simple`

### `std.io` (`io.rl`)
- `(key, value) format_kv`
- `(key, value) println_kv`
- `(label, message) eprintln_labeled`

### `std.lib` (`lib.rl`)
標準ライブラリのルートインデックスと `() std_version` を提供します。

## 使用例

```restrict
import std.math.{clamp, gcd}
import std.list.{list_range, list_sum}

export fun main: () -> Int32 = {
    val total = (1, 6) list_range |> list_sum;
    val bounded = (total, 0, 10) clamp;
    bounded + ((12, 8) gcd)
}
```
