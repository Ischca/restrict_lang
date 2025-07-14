# クイックスタート

Restrict Languageをすぐに始めるためのガイドです。10分以内に最初のプログラムを実行できます！

## インストール

### 方法1: Homebrewを使用（推奨）

```bash
# Homebrewタップを追加
brew tap restrict-lang/tap

# Restrict Languageをインストール
brew install restrict-lang
```

### 方法2: インストールスクリプトを使用

```bash
# インストールスクリプトをダウンロードして実行
curl -fsSL https://raw.githubusercontent.com/restrict-lang/restrict_lang/main/install.sh | sh
```

### 方法3: ソースからビルド

```bash
# リポジトリをクローン
git clone https://github.com/restrict-lang/restrict_lang
cd restrict_lang

# ビルド（Rustが必要）
cargo build --release

# パスに追加
export PATH="$PWD/target/release:$PATH"
```

## 最初のプログラム

### 1. ファイルを作成

`hello.rl`という名前のファイルを作成します：

```restrict
// hello.rl
fun main = {
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
fun main = {
    val x = 42
    val y = x    // xはyに移動
    // val z = x // エラー: xはすでに使用されている
    
    val mut counter = 0
    counter = counter + 1  // 可変変数は再利用可能
    counter = counter + 1
    
    counter |> println
}
```

### OSV構文と関数

```restrict
// functions.rl
fun double = x:Int { x * 2 }
fun add = x:Int, y:Int { x + y }

fun main = {
    // OSV構文
    val result1 = 21 double         // 42
    val result2 = (10, 20) add      // 30
    
    // 関数の連鎖
    val result3 = 5 double add(2, _) double  // 24
    
    result3 |> println
}
```

### パターンマッチング

```restrict
// patterns.rl
fun factorial = n:Int -> Int {
    n match {
        0 => { 1 }
        1 => { 1 }
        _ => { n * (n - 1) factorial }
    }
}

fun process_option = opt:Option<Int> {
    opt match {
        Some(value) => { value double }
        None => { 0 }
    }
}

fun main = {
    val result = 5 factorial
    result |> println  // 120
    
    val some_value = Some(21)
    val doubled = some_value process_option
    doubled |> println  // 42
}
```

### リストとラムダ

```restrict
// lists.rl
fun main = {
    val numbers = [1, 2, 3, 4, 5]
    
    // ラムダ式を使ったフィルタとマップ
    val evens = numbers 
        |> filter(|x| x % 2 == 0)
        |> map(|x| x * x)
    
    evens match {
        [] => { "No even numbers" |> println }
        [head | _] => { head |> println }  // 最初の要素を表示
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
# └── README.md
```

### プロジェクトをビルドして実行

```bash
# プロジェクトをビルド
warder build

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
warder add ./path/to/local/package
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
export PATH="$HOME/.local/bin:$PATH"
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

Restrict Languageへようこそ！🎉