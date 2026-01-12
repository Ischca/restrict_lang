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
// ファイルI/O組み込み関数 / File I/O built-in functions:
// - file_open: (path: String, flags: Int) -> Int
//     flags: 0 = 読み取り / read, 1 = 書き込み+作成 / write+create
// - file_read: (fd: Int, len: Int) -> String
// - file_write: (fd: Int, content: String) -> Int
// - file_close: (fd: Int) -> Int

// 文字列を出力する
// Print a string
export fun hello: () -> Unit = {
    "Hello, World!" println
}

// 数値を出力する
// Print a number
export fun show_number: (n: Int) -> Unit = {
    n print_int
}

// 入力を読み取る (WASI対応)
// Read input from stdin (WASI-enabled)
export fun echo_line: () -> Unit = {
    val input = read_line;
    "You entered: " println;
    input println
}

// エラー出力に書き込む (WASI対応)
// Write to stderr (WASI-enabled)
export fun log_error: (msg: String) -> Unit = {
    msg eprint
}

// ファイル読み取り (WASI対応)
// Read entire file contents
export fun read_file: (path: String) -> String = {
    val fd = (path, 0) file_open;
    val content = (fd, 4096) file_read;
    fd file_close;
    content
}

// ファイル書き込み (WASI対応)
// Write content to file
export fun write_file: (path: String, content: String) -> Unit = {
    val fd = (path, 1) file_open;
    (fd, content) file_write;
    fd file_close;
    Unit
}

// ファイルに追記
// Append to file
export fun append_file: (path: String, content: String) -> Unit = {
    val fd = (path, 1) file_open;
    (fd, content) file_write;
    fd file_close;
    Unit
}
