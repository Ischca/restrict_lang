# はじめに

**Restrict Language**へようこそ — メモリ安全性と最適なパフォーマンスを保証するアフィン型システムを備えた、WebAssembly向けのモダンなプログラミング言語です。

## Restrict Languageとは？

Restrict Languageは、以下のユニークな特徴を持つ静的型付けコンパイル言語です：

- 日本語の構文に着想を得た**OSV（目的語-主語-動詞）語順**
- 各値が最大1回しか使用されないことを保証する**アフィン型システム**
- `freeze`と`clone`操作を持つ**プロトタイプベースのレコード**
- ガベージコレクタを持たない**WebAssemblyファースト**設計
- WebAssemblyコンポーネントモデルとWITによる**相互運用性**

## 核となる哲学

### 名前1つ = 実体1つ

Restrict Languageでは、すべてのバインディングが単一の一意な実体を表します。この原則がアフィン型システムを駆動し、予測可能なリソース管理を実現し、use-after-moveやデータ競合などの一般的なプログラミングエラーを防ぎます。

{{#include ../includes/philosophy-example.md}}

### GCなしでのメモリ安全性

アフィン型と明示的なメモリ管理を活用することで、Restrict Languageはガベージコレクションのオーバーヘッドなしにメモリ安全性を実現します。これにより以下の用途に最適です：

- 高性能WebAssemblyアプリケーション
- リソース制約のある環境
- リアルタイムシステム
- ブロックチェーンとスマートコントラクト

## 主要機能

### 1. OSV構文

日本語文法に着想を得て、Restrict Languageは目的語-主語-動詞の順序を使用します：

{{#include ../includes/osv-intro.md}}

### 2. アフィン型

各値は最大1回しか参照できず、エイリアシングバグを防ぎます：

```restrict
fun consume: (x: String) -> () = {
    x |> println
}

fun main: () = {
    val msg = "Hello"
    msg |> consume
    // msgはここでは使用できません
}
```

### 3. プロトタイプベース継承

クラスの代わりに、Restrictは明示的なクローンとフリーズを持つプロトタイプを使用します：

```restrict
record Car {
    wheels: Int32
    color: String
}

fun main: () = {
    val base_car = Car { wheels: 4, color: "未設定" }
    val my_car = base_car.clone { color: "赤" }
    val frozen_car = my_car freeze
    frozen_car
}
```

### 4. WebAssembly統合

Restrictは以下のファーストクラスサポートを持つWebAssemblyに直接コンパイルされます：

- WASI（WebAssemblyシステムインターフェース）
- コンポーネントモデル
- WIT（WebAssemblyインターフェースタイプ）
- 言語間相互運用性

## はじめよう

準備はできましたか？以下から始めましょう：

1. [インストール](./getting-started/installation.md) - 開発環境のセットアップ
2. [Hello World](./getting-started/hello-world.md) - 最初のRestrictプログラムを書く
3. [Warderパッケージマネージャー](./getting-started/warder.md) - パッケージ管理について学ぶ

## サンプルプログラム

Restrict Languageの雰囲気を味わってください：

```restrict
// アフィン型を持つ関数を定義
fun greet: (name: String) -> String = {
    "こんにちは、" + name + "さん！"
}

fun add: (x: Int32, y: Int32) -> Int32 = {
    x + y
}

// メインエントリポイント
fun main: () = {
    val name = "世界"
    val message = name |> greet
    message |> println

    val total = (20, 22) add
    total |> print_int
}
```

## なぜRestrict Language？

- **パフォーマンス**: ゼロコスト抽象化とGCオーバーヘッドなし
- **安全性**: アフィン型がコンパイル時に一般的なメモリバグを防ぐ
- **シンプルさ**: 関数型プログラミングに着想を得た明確な構文
- **相互運用性**: 既存のWebAssemblyエコシステムとのシームレスな統合
- **モダンなツール**: 組み込みパッケージマネージャー、LSPサポート、VS Code拡張

WebAssemblyプログラミングの未来を一緒に築きましょう！
