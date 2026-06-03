# クイックスタート

Restrict Languageをすぐに始めるためのガイドです。10分以内に最初のプログラムを実行できます！

## インストール

v0.0.1の検証では、まずソースからビルドする手順を使います。

```bash
# リポジトリをクローン
git clone https://github.com/restrict-lang/restrict_lang
cd restrict_lang

# ビルド（Rustとmiseが必要）
mise exec -- cargo build --workspace --release

# パスに追加
export PATH="$PWD/target/release:$PWD/warder/target/release:$PATH"
```

ワークスペース全体をビルドした場合、通常は`target/release`に`restrict_lang`と`warder`が生成されます。Warderだけを別ディレクトリでビルドした場合は、`warder/target/release`もPATHに含めてください。

## 最初のプログラム

### 1. ファイルを作成

`hello.rl`という名前のファイルを作成します：

```restrict
// hello.rl
fun main: () = {
    "Hello, Restrict Language!" |> println
}
```

### 2. コンパイルして実行

```bash
# WebAssemblyにコンパイル
restrict_lang hello.rl

# wasmtimeで実行（wasmtimeがインストールされている場合）
wasmtime hello.wat

# または、生成されたWATファイルを確認
cat hello.wat
```

## 基本的な例

### 変数とアフィン型

```restrict
// affine.rl
fun main: () = {
    val x = "owned value"
    val y = x    // xはyに移動
    // val z = x // エラー: xはすでに使用されている
    y |> println

    mut val counter = 0
    counter = counter + 1  // 可変変数は再利用可能
    counter = counter + 1

    counter |> print_int
}
```

### OSV構文と関数

```restrict
// functions.rl
fun double: (x: Int32) -> Int32 = {
    x * 2
}

fun add: (x: Int32, y: Int32) -> Int32 = {
    x + y
}

fun main: () = {
    // OSV構文
    val result1 = 21 |> double      // 42
    val result2 = (10, 20) add      // 30

    // 関数の連鎖と複数引数
    val result3 = result1 |> double // 84
    val total = (result2, result3) add

    total |> print_int
}
```

### パターンマッチング

```restrict
// patterns.rl
fun double: (x: Int32) -> Int32 = {
    x * 2
}

fun factorial: (n: Int32) -> Int32 = {
    n match {
        0 => { 1 }
        1 => { 1 }
        _ => { n * ((n - 1) |> factorial) }
    }
}

fun process_option: (opt: Option<Int32>) -> Int32 = {
    opt match {
        Some(value) => { value |> double }
        None => { 0 }
    }
}

fun main: () = {
    val result = 5 |> factorial
    result |> print_int  // 120
}
```

### リストとラムダ

```restrict
// lists.rl
fun main: () = {
    val square = |x: Int32| x * x
    val value = 6 |> square
    value |> print_int

    val numbers = [1, 2, 3, 4, 5]
    numbers match {
        [] => { "No numbers" |> println }
        [head | _] => { head |> print_int }  // 最初の要素を表示
    }
}
```

## Warderを使ったプロジェクト管理

### 新しいプロジェクトを作成

```bash
# Warderで新しいプロジェクトを作成
warder new my-project
cd my-project

# プロジェクト構造
tree .
# .
# ├── package.rl.toml
# ├── src/
# │   └── main.rl
# ├── tests/
# │   └── main_test.rl
# ├── README.md
# └── .gitignore
```

### プロジェクトをビルドして実行

```bash
# プロジェクトをビルド
warder build
# 既定の成果物:
# dist/my-project-0.1.0.wat
# dist/my-project-0.1.0.wasm
# dist/my-project-0.1.0.rgc

# プログラムを実行
warder run

# テストを実行
warder test
```

### 依存関係を追加

```bash
# 依存関係を追加
warder add some-package

# ローカル依存関係を追加
warder add local-package --path ./path/to/local/package
```

## 次のステップ

おめでとうございます！Restrict Languageの基本を学びました。さらに学ぶには：

1. **[言語ガイド](../guide/README.md)** - 言語機能の詳細な説明
2. **[アフィン型](../guide/affine-types.md)** - Restrict Languageの中核となる型システム
3. **[OSV構文](../guide/osv-order.md)** - 関数合成のための独自の構文
4. **[標準ライブラリ](../reference/stdlib.md)** - 利用可能な関数とモジュール

## トラブルシューティング

### restrict_langコマンドが見つからない

PATHに実行ファイルが含まれていることを確認してください：

```bash
# インストール場所を確認
which restrict_lang

# PATHに追加（必要に応じて）
export PATH="$PWD/target/release:$PWD/warder/target/release:$PATH"
```

### WebAssemblyランタイムがない

wasmtimeをインストールします：

```bash
curl https://wasmtime.dev/install.sh -sSf | bash
```

### コンパイルエラー

エラーメッセージは通常、問題を明確に示します：

```restrict
val x = 42
val y = x
val z = x  // エラー: Variable 'x' has already been used
```

## コミュニティとサポート

- **GitHub**: [https://github.com/restrict-lang/restrict_lang](https://github.com/restrict-lang/restrict_lang)
- **ドキュメント**: [https://restrict-lang.github.io/restrict_lang/](https://restrict-lang.github.io/restrict_lang/)
- **Issues**: バグ報告や機能リクエストはGitHub Issuesへ

Restrict Languageへようこそ！
