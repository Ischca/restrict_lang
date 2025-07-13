// 標準ライブラリ: メインエントリポイント
// Standard Library: Main Entry Point

// 全ての標準ライブラリモジュールをエクスポート
export import math.*
export import string.*
export import list.*
export import option.*
export import io.*

// Preludeは自動的にインポートされる
import prelude.*

// 標準ライブラリのバージョン情報
val STD_VERSION = "0.1.0"

// ライブラリの初期化関数
fun std_init() {
    // 標準ライブラリの初期化処理
    // 現時点では何もしない
    Unit
}

// ライブラリの情報を出力
fun std_info() {
    println("Restrict Language Standard Library")
    println("Version: " + STD_VERSION)
    println("Modules: math, string, list, option, io")
}