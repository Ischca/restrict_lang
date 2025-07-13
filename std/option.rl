// 標準ライブラリ: Option型操作
// Standard Library: Option Operations

// Optionが値を持つかどうか
fun<T> option_is_some(opt: Option<T>) {
    match opt {
        Some(_) => true,
        None => false
    }
}

// Optionが空かどうか
fun<T> option_is_none(opt: Option<T>) {
    match opt {
        Some(_) => false,
        None => true
    }
}

// Optionから値を取得（デフォルト値付き）
fun<T> option_unwrap_or(opt: Option<T>, default: T) {
    match opt {
        Some(value) => value,
        None => default
    }
}

// Optionの値に関数を適用
fun<T, U> option_map(opt: Option<T>, f: (T) -> U) {
    match opt {
        Some(value) => Some(f(value)),
        None => None
    }
}

// Optionをフラット化
fun<T> option_flatten(opt: Option<Option<T>>) {
    match opt {
        Some(inner) => inner,
        None => None
    }
}

// Optionの値に関数を適用（結果もOption）
fun<T, U> option_and_then(opt: Option<T>, f: (T) -> Option<U>) {
    match opt {
        Some(value) => f(value),
        None => None
    }
}

// 2つのOptionを結合
fun<T, U, V> option_zip(a: Option<T>, b: Option<U>, f: (T, U) -> V) {
    match a {
        Some(val_a) => match b {
            Some(val_b) => Some(f(val_a, val_b)),
            None => None
        },
        None => None
    }
}

// Optionをリストに変換
fun<T> option_to_list(opt: Option<T>) {
    match opt {
        Some(value) => [value],
        None => []
    }
}

// リストの最初の要素をOptionで取得
fun<T> list_to_option(list: List<T>) {
    match list {
        [head | _] => Some(head),
        [] => None
    }
}