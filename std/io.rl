// 標準ライブラリ: IO操作
// Standard Library: IO Operations
//
// WASI (WebAssembly System Interface) を使用した入出力機能を提供します。
// Provides I/O functionality using WASI (WebAssembly System Interface).
//
// 利用可能な組み込み関数 / Available built-in functions:
// - println: (s: String) -> Unit   - 文字列を標準出力に出力（改行付き）
// - print_int: (n: Int) -> Unit    - 整数を標準出力に出力（改行付き）
// - eprint: (s: String) -> Unit    - 文字列を標準エラー出力に出力
// - read_line: () -> String        - 標準入力から1行読み取る
//
// 使用例 / Example usage:

// 文字列を出力する
// Print a string
fun hello: () -> Unit = {
    "Hello, World!" println
}

// 数値を出力する
// Print a number
fun show_number: (n: Int) -> Unit = {
    n print_int
}

// 入力を読み取る (WASI対応)
// Read input from stdin (WASI-enabled)
fun echo_line: () -> Unit = {
    val input = read_line;
    "You entered: " println;
    input println
}

// エラー出力に書き込む (WASI対応)
// Write to stderr (WASI-enabled)
fun log_error: (msg: String) -> Unit = {
    msg eprint
}

// ファイル読み取り（今後実装予定）
// File reading (to be implemented)
fun read_file: (path: String) -> String = {
    // TODO: WASIのpath_open, fd_readを使用
    ""
}

// ファイル書き込み（今後実装予定）
// File writing (to be implemented)
fun write_file: (path: String, content: String) -> Unit = {
    // TODO: WASIのpath_open, fd_writeを使用
    Unit
}

// メイン関数の例
// Example main function
fun main: () -> Int = {
    "Welcome to Restrict Lang!" println;
    0
}
