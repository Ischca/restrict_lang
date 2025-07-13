// 標準ライブラリ: リスト操作
// Standard Library: List Operations

// リストが空かどうか
fun<T> list_is_empty(list: List<T>) {
    match list {
        [] => true,
        _ => false
    }
}

// リストの最初の要素を取得
fun<T> list_head(list: List<T>) {
    match list {
        [head | _] => Some(head),
        [] => None
    }
}

// リストの末尾を取得
fun<T> list_tail(list: List<T>) {
    match list {
        [_ | tail] => Some(tail),
        [] => None
    }
}

// リストの最後の要素を取得
fun<T> list_last(list: List<T>) {
    match list {
        [x] => Some(x),
        [_ | tail] => list_last(tail),
        [] => None
    }
}

// リストを逆順にする
fun<T> list_reverse(list: List<T>) {
    list_reverse_helper(list, [])
}

fun<T> list_reverse_helper(list: List<T>, acc: List<T>) {
    match list {
        [] => acc,
        [head | tail] => list_reverse_helper(tail, [head | acc])
    }
}

// リストに要素を追加（先頭）
fun<T> list_prepend(item: T, list: List<T>) {
    [item | list]
}

// リストに要素を追加（末尾）
fun<T> list_append(list: List<T>, item: T) {
    list_reverse(list_prepend(item, list_reverse(list)))
}

// 2つのリストを連結
fun<T> list_concat(a: List<T>, b: List<T>) {
    match a {
        [] => b,
        [head | tail] => [head | list_concat(tail, b)]
    }
}

// リストの要素数を数える
fun<T> list_count(list: List<T>) {
    list_count_helper(list, 0)
}

fun<T> list_count_helper(list: List<T>, acc: Int) {
    match list {
        [] => acc,
        [_ | tail] => list_count_helper(tail, acc + 1)
    }
}

// リストから指定したインデックスの要素を取得
fun<T> list_at(list: List<T>, index: Int) {
    then index < 0 {
        None
    } else {
        list_at_helper(list, index)
    }
}

fun<T> list_at_helper(list: List<T>, index: Int) {
    match list {
        [] => None,
        [head | tail] => then index == 0 {
            Some(head)
        } else {
            list_at_helper(tail, index - 1)
        }
    }
}

// リストの要素をフィルタリング
fun<T> list_filter(list: List<T>, predicate: (T) -> Bool) {
    match list {
        [] => [],
        [head | tail] => then predicate(head) {
            [head | list_filter(tail, predicate)]
        } else {
            list_filter(tail, predicate)
        }
    }
}

// リストの各要素に関数を適用
fun<T, U> list_map(list: List<T>, f: (T) -> U) {
    match list {
        [] => [],
        [head | tail] => [f(head) | list_map(tail, f)]
    }
}

// リストを畳み込み（左から）
fun<T, U> list_fold_left(list: List<T>, acc: U, f: (U, T) -> U) {
    match list {
        [] => acc,
        [head | tail] => list_fold_left(tail, f(acc, head), f)
    }
}

// リストを畳み込み（右から）
fun<T, U> list_fold_right(list: List<T>, acc: U, f: (T, U) -> U) {
    match list {
        [] => acc,
        [head | tail] => f(head, list_fold_right(tail, acc, f))
    }
}