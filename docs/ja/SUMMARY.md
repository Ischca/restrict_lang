# 目次

[はじめに](./introduction.md)

# 入門

- [インストール](./getting-started/installation.md)
- [Hello World](./getting-started/hello-world.md)
- [Warderパッケージマネージャー](./getting-started/warder.md)

# 言語ガイド

- [構文の基礎](./guide/syntax.md)
  - [OSV語順](./guide/osv-order.md)
  - [変数と可変性](./guide/variables.md)
  - [関数](./guide/functions.md)
  - [制御フロー](./guide/control-flow.md)
  
- [型システム](./guide/types.md)
  - [プリミティブ型](./guide/primitive-types.md)
  - [アフィン型](./guide/affine-types.md)
  - [レコードとプロトタイプ](./guide/records.md)
  - [型推論](./guide/type-inference.md)
  
- [所有権と借用](./guide/ownership.md)
  - [単一所有権ルール](./guide/single-ownership.md)
  - [freezeとclone](./guide/freeze-clone.md)
  - [コンテキストバインディング](./guide/context-binding.md)

# 高度な機能

- [ジェネリックプログラミング](./advanced/generics.md)
  - [型パラメータ](./advanced/type-parameters.md)
  - [型境界](./advanced/type-bounds.md)
  - [単相化](./advanced/monomorphization.md)
  
- [ラムダ式](./advanced/lambdas.md)
  - [クロージャ](./advanced/closures.md)
  - [高階関数](./advanced/higher-order.md)
  
- [パターンマッチング](./advanced/patterns.md)
  - [match式](./advanced/match.md)
  - [リストパターン](./advanced/list-patterns.md)
  - [分解](./advanced/destructuring.md)

# Warderエコシステム

- [パッケージ管理](./warder/packages.md)
  - [プロジェクト作成](./warder/new-project.md)
  - [依存関係](./warder/dependencies.md)
  - [パッケージ公開](./warder/publishing.md)
  
- [Cageフォーマット](./warder/cage.md)
  - [構造](./warder/cage-structure.md)
  - [ABIハッシュ](./warder/abi-hash.md)
  - [セキュリティ](./warder/security.md)
  
- [WebAssembly統合](./warder/wasm.md)
  - [WITインターフェース](./warder/wit.md)
  - [コンポーネントモデル](./warder/components.md)
  - [外部関数](./warder/foreign.md)

# 標準ライブラリ

- [概要](./std/overview.md)
- [プレリュード](./std/prelude.md)
- [コレクション](./std/collections.md)
  - [リスト](./std/list.md)
  - [オプション](./std/option.md)
- [文字列操作](./std/string.md)
- [I/O操作](./std/io.md)
- [数学関数](./std/math.md)

# ツールと開発

- [IDE サポート](./tools/ide.md)
  - [VS Code拡張](./tools/vscode.md)
  - [Language Server Protocol](./tools/lsp.md)
  
- [デバッグ](./tools/debugging.md)
- [テスト](./tools/testing.md)
- [ベンチマーク](./tools/benchmarking.md)

# リファレンス

- [言語リファレンス](./reference/index.md)
  - [文法](./reference/grammar.md)
  - [キーワード](./reference/keywords.md)
  - [演算子](./reference/operators.md)
  
- [APIドキュメント](./api/index.md)
  - [コンパイラAPI](./api/compiler.md)
  - [ランタイムAPI](./api/runtime.md)
  
- [エラーメッセージ](./reference/errors.md)

# 貢献

- [開発環境のセットアップ](./contributing/setup.md)
- [アーキテクチャ](./contributing/architecture.md)
- [コーディングガイドライン](./contributing/guidelines.md)
- [テスト](./contributing/testing.md)

---

[付録A: サンプルコード](./appendix/examples.md)
[付録B: FAQ](./appendix/faq.md)
[付録C: 用語集](./appendix/glossary.md)