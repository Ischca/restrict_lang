// 標準ライブラリ: Prelude（自動インポート）
// Standard Library: Prelude (Auto-imported)

// 基本的な数学関数
import math.{abs, max, min, pow}

// 基本的なリスト操作
import list.{list_is_empty, list_head, list_tail, list_length, list_append}

// 基本的なOption操作
import option.{option_is_some, option_is_none, option_unwrap_or}

// 基本的なIO操作
import io.{print, print_int, debug_print}

// 基本的なユーティリティ関数

// 恒等関数
fun<T> identity(x: T) {
    x
}

// 関数合成
fun<A, B, C> compose(f: (B) -> C, g: (A) -> B) {
    |x| f(g(x))
}

// 値を無視する
fun<T> ignore(x: T) {
    Unit
}

// 真偽値の否定
fun not(b: Bool) {
    then b {
        false
    } else {
        true
    }
}

// 論理積
fun and(a: Bool, b: Bool) {
    then a {
        b
    } else {
        false
    }
}

// 論理和
fun or(a: Bool, b: Bool) {
    then a {
        true
    } else {
        b
    }
}

// 排他的論理和
fun xor(a: Bool, b: Bool) {
    then a {
        not(b)
    } else {
        b
    }
}

// 値が等しいかどうか（基本型用）
fun<T> eq(a: T, b: T) {
    // TODO: 型に応じた比較を実装
    true
}

// 値が異なるかどうか
fun<T> ne(a: T, b: T) {
    not(eq(a, b))
}

// パニック（プログラム終了）
fun panic(message: String) {
    eprintln(message)
    // TODO: プログラムを異常終了
    Unit
}

// アサーション
fun assert(condition: Bool, message: String) {
    then not(condition) {
        panic(message)
    } else {
        Unit
    }
}

// デバッグ用のアサーション
fun debug_assert(condition: Bool, message: String) {
    // デバッグビルドでのみ有効
    assert(condition, message)
}

// 条件付き実行
fun<T> when(condition: Bool, action: () -> T, default: T) {
    then condition {
        action()
    } else {
        default
    }
}