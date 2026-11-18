// Standard Library: Root Index
//
// Bundled pure-Restrict standard library modules:
// - std.prelude: identity_val, boolean and comparison helpers
// - std.math: clamp, signum, gcd, lcm, parity, Int64 and Float64 helpers
// - std.option: option_or, option_unwrap_or_else, option_and_then, option_ok_or,
//   option_flatten, option_zip_with
// - std.result: result_is_ok, result_is_err, result_unwrap_or,
//   result_unwrap_or_else, result_map, result_map_err, result_and_then,
//   result_ok, result_err
// - std.list: IndexedInt, list_any, list_all, list_find, list_take, list_drop,
//   list_range, list_sum, list_sum_i64, list_sum_f, list_product,
//   list_enumerate_from, list_zip_with
// - std.string: string_is_empty, string_non_empty, string_repeat,
//   string_pad_left, string_join_simple
// - std.io: format_kv, println_kv, eprintln_labeled
//
// Host capability modules outside the pure-Restrict v0.1.0 default surface:
// - std.filesystem, std.http, std.database

pub fun std_version: () -> String = {
    "0.1.0"
}
