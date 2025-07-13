// 標準ライブラリ: 数学関数
// Standard Library: Math Functions

// 基本的な数学関数
fun abs(x: Int) {
    then x < 0 {
        -x
    } else {
        x
    }
}

fun max(a: Int, b: Int) {
    then a > b {
        a
    } else {
        b
    }
}

fun min(a: Int, b: Int) {
    then a < b {
        a
    } else {
        b
    }
}

// Float版
fun abs_f(x: Float) {
    then x < 0.0 {
        -x
    } else {
        x
    }
}

fun max_f(a: Float, b: Float) {
    then a > b {
        a
    } else {
        b
    }
}

fun min_f(a: Float, b: Float) {
    then a < b {
        a
    } else {
        b
    }
}

// 累乗関数（簡単な実装）
fun pow(base: Int, exp: Int) {
    mut result = 1
    mut i = 0
    while i < exp {
        result = result * base
        i = i + 1
    }
    result
}

// 階乗
fun factorial(n: Int) {
    then n <= 1 {
        1
    } else {
        n * factorial(n - 1)
    }
}