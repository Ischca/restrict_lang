// 標準ライブラリ: IO操作
// Standard Library: IO Operations

// 基本的な出力
fun print(s: String) {
    // TODO: WASIのfd_writeを使用
    println(s)
}

// 数値の出力
fun print_int(n: Int) {
    println(int_to_string(n))
}

// Float値の出力
fun print_float(f: Float) {
    // TODO: float_to_string関数を実装
    println("0.0")
}

// デバッグ出力
fun<T> debug_print(value: T) {
    // TODO: 型に応じた文字列表現を生成
    println("debug")
}

// 改行なしで文字列を出力
fun print_no_newline(s: String) {
    // TODO: WASIのfd_writeを直接使用
    print(s)
}

// エラー出力（stderr）
fun eprint(s: String) {
    // TODO: stderrに出力
    println(s)
}

// エラー出力（改行付き）
fun eprintln(s: String) {
    eprint(s)
    eprint("\n")
}

// 入力読み取り（今後実装予定）
fun read_line() {
    // TODO: WASIのfd_readを使用
    ""
}

// ファイル読み取り（今後実装予定）
fun read_file(path: String) {
    // TODO: ファイルシステムアクセス
    ""
}

// ファイル書き込み（今後実装予定）
fun write_file(path: String, content: String) {
    // TODO: ファイルシステムアクセス
    Unit
}