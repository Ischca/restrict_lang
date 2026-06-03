# Hello, World!

最初の Restrict Language プログラムを書きましょう。このページのコードブロックは、v0.0.1 の current surface に合わせた例です。

## ファイルの作成

作業用ディレクトリに `hello.rl` を作成します。

```restrict
// 最初の Restrict Language プログラム
fun main: () -> () = {
    "Hello, World!" |> println
}
```

このプログラムの構成要素は次のとおりです。

1. `fun main: () -> () = { ... }` は引数なしで unit を返すエントリーポイントです。
2. `"Hello, World!"` は文字列リテラルです。
3. `|>` は値を左から右へ渡す OSV パイプ演算子です。
4. `println` は標準出力へ文字列を出力する現在の標準ライブラリ関数です。

## 実行

リポジトリ内で試す場合は、コンパイラを直接実行できます。

```bash
mise exec -- cargo run --bin restrict_lang hello.rl
```

出力は次のようになります。

```text
Hello, World!
```

## OSV 構文

Restrict Language は Object-Subject-Verb の語順を使います。引数は関数名の前に置きます。

```restrict
// 単一引数
"Hello, World!" |> println

// 複数引数
(10, 20) add
```

Rust や JavaScript のように関数名を先に書く呼び出し形式は、v0.0.1 の Restrict 構文ではありません。

## 少し拡張する

```restrict
fun greet: (name: String) -> () = {
    "Hello, " + name + "!" |> println
}

fun main: () -> () = {
    "Restrict" |> greet
}
```

文字列結合には現在の文字列演算である `+` を使います。

## アフィン型の基本

Restrict の値は、基本的に最大 1 回だけ使えます。

```restrict
fun main: () -> () = {
    val message = "このメッセージは一度だけ使えます"

    message |> println

    // もう一度使うと、アフィン型チェックでエラーになります。
    // message |> println
}
```

同じ束縛を複数回使う必要がある場合は、明示的に可変束縛にします。

```restrict
fun main: () -> () = {
    mut val greeting = "Hello"

    greeting |> println
    greeting |> println
}
```

## 関数を分ける

```restrict
fun greeting_word: (hour: Int32) -> String = {
    hour < 12 then {
        "おはよう"
    } else {
        hour < 18 then {
            "こんにちは"
        } else {
            "こんばんは"
        }
    }
}

fun greet_with_time: (name: String, hour: Int32) -> () = {
    val word = hour |> greeting_word
    word + "、" + name + "さん！" |> println
}

fun main: () -> () = {
    ("Taro", 9) greet_with_time
    ("Yuki", 15) greet_with_time
    ("Hana", 20) greet_with_time
}
```

複数引数の関数は、タプルを関数名の前に置いて呼び出します。

## Result の基本

ファイル I/O などの実用 API はまだ v0.0.1 の current standard-library surface ではありません。エラー処理の形は、現在の構文では次のように `Result` と `match` で表します。

```restrict
fun greeting_for_hour: (hour: Int32) -> Result<String, String> = {
    hour < 0 then {
        Err("時刻が不正です")
    } else {
        Ok("Hello from Restrict")
    }
}

fun main: () -> () = {
    val result = 9 |> greeting_for_hour

    result match {
        Ok(message) => { message |> println }
        Err(error) => { error |> println }
    }
}
```

## v0.0.1 ではまだ current example ではないもの

次の内容は将来の標準ライブラリやツール設計で扱う予定です。このページでは、v0.0.1 の現在のコード例としては示しません。

- 標準入力を読む対話プログラム
- ファイルを読み書きするチュートリアル
- テスト属性や専用テストハーネス
- 標準ライブラリの集約 import

## 次のステップ

1. [OSV語順ガイド](../guide/osv-order.md) - パイプ演算子をマスター
2. [型システム](../guide/types.md) - アフィン型を深く理解
3. [Warderガイド](../guide/warder.md) - 依存関係の管理を学ぶ
