# Hello, World!

最初のRestrict Languageプログラムを書きましょう！このチュートリアルでは、伝統的な「Hello, World!」プログラムを作成し、Restrict Languageの基本的な構文とツールに慣れていきます。

## 新しいプロジェクトの作成

Warderパッケージマネージャーを使用して新しいプロジェクトを作成します：

```bash
warder new hello-world
cd hello-world
```

これにより、基本的なプロジェクト構造が作成されます：

```
hello-world/
├── package.rl.toml    # パッケージ設定
├── src/
│   └── main.rl       # メインソースファイル
└── tests/
    └── main_test.rl  # テストファイル
```

## コードの記述

`src/main.rl`を開いて、以下のコードに置き換えます：

{{#include ../../includes/hello-world-main.md}}

このシンプルなプログラムを分解してみましょう：

1. `fn main()` - すべてのRestrictプログラムのエントリーポイント
2. `"Hello, World!"` - 文字列リテラル
3. `|>` - パイプ演算子（OSV構文の核心）
4. `println` - 標準出力に出力する関数

## プログラムの実行

プログラムを実行するには：

```bash
warder run
```

以下の出力が表示されるはずです：

```
Hello, World!
```

## OSV構文の理解

Restrict Languageは、日本語の文法に触発されたObject-Subject-Verb（OSV）の語順を使用します。従来のプログラミング言語との比較：

```restrict
// Restrict Language (OSV)
"Hello, World!" |> println

// 従来のスタイル (SVO)
// println("Hello, World!")
```

データが左から右に流れ、変換のチェーンが視覚的に明確になります。

## プログラムの拡張

より興味深いものにしてみましょう：

{{#include ../../includes/hello-extended.md}}

## ユーザー入力の追加

対話的にしてみましょう：

{{#include ../../includes/hello-interactive.md}}

## アフィン型の実演

Restrict Languageのアフィン型システムは、値が最大1回しか使用できないことを保証します：

```restrict
fn main() {
    let message = "このメッセージは一度しか使えません"
    
    // 最初の使用 - OK
    message |> println
    
    // 二回目の使用 - コンパイルエラー！
    // message |> println  // エラー: messageは既に消費されています
    
    // 複数回使用する必要がある場合は、cloneを使用
    let greeting = "Hello"
    let greeting_copy = clone greeting
    
    greeting |> println       // OK
    greeting_copy |> println  // OK
}
```

## 関数の作成

プログラムをよりモジュラーにしましょう：

```restrict
// カスタムグリーティング関数
fn greet(name: String) {
    "Hello, " ++ name ++ "!" |> println
}

// 時刻に基づいたグリーティング
fn greetWithTime(name: String, hour: i32) {
    let greeting = if hour < 12 {
        "おはよう"
    } else if hour < 18 {
        "こんにちは"
    } else {
        "こんばんは"
    }
    
    greeting ++ "、" ++ name ++ "さん！" |> println
}

fn main() {
    "World" |> greet
    
    "Taro" |> greetWithTime(9)   // おはよう、Taroさん！
    "Yuki" |> greetWithTime(15)  // こんにちは、Yukiさん！
    "Hana" |> greetWithTime(20)  // こんばんは、Hanaさん！
}
```

## エラー処理

Restrict LanguageはResult型を使用した明示的なエラー処理を推奨します：

```restrict
use std::fs::readFile;

fn readGreeting(filename: String) -> Result<String, String> {
    filename
        |> readFile
        |> mapErr(|e| "ファイルを読み込めませんでした: " ++ e.toString())
}

fn main() {
    let result = "greeting.txt" |> readGreeting
    
    match result {
        Ok(content) => content |> println,
        Err(error) => error |> println
    }
}
```

## WebAssemblyへのコンパイル

WebAssemblyモジュールとしてプログラムをコンパイル：

```bash
warder build --release
```

これにより、`target/wasm32-wasi/release/hello-world.wasm`にWebAssemblyモジュールが作成されます。

## テストの作成

`tests/main_test.rl`にテストを追加：

{{#include ../../includes/test-example.md}}

テストを実行：

```bash
warder test
```

## 次のステップ

おめでとうございます！最初のRestrict Languageプログラムを作成しました。ここで学んだこと：

- ✅ Warderでプロジェクトを作成
- ✅ OSV構文でコードを記述
- ✅ プログラムを実行
- ✅ アフィン型の理解
- ✅ 関数とエラー処理
- ✅ WebAssemblyへのコンパイル
- ✅ テストの記述

次に探求すること：

1. [OSV語順ガイド](../guide/osv-order.md) - パイプ演算子をマスター
2. [型システム](../guide/types.md) - アフィン型を深く理解
3. [Warderガイド](../guide/warder.md) - 依存関係の管理を学ぶ

## 演習

理解を深めるために、以下を試してみてください：

1. ユーザーから2つの数値を読み取り、その合計を出力するプログラムを作成
2. リストの要素を処理するためにパイプ演算子を使用
3. カスタムエラー型でエラー処理を実装

<details>
<summary>演習の解答</summary>

{{#include ../../includes/exercises-solution.md}}

</details>

楽しいコーディングを！🦀