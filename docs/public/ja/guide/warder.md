# Warderパッケージマネージャー

WarderはRestrict Languageのプロジェクト作成、依存関係管理、ビルド、実行、テストをまとめるツールです。v0.0.1で公開対象として扱うコマンドは、`new`、`init`、`add`、`remove`、`build`、`run`、`test`、`publish`、`wrap`、`unwrap`、`doctor`です。

## 主要概念

### Cageフォーマット（.rgc）

CageはWarderのパッケージ成果物です。`warder build`は既定で`dist/<name>-<version>.wat`、`dist/<name>-<version>.wasm`、`dist/<name>-<version>.rgc`を生成します。

### ヴォールト（restrict-lock.toml）

`restrict-lock.toml`は依存関係のロックファイルです。Warderは依存関係を解決した結果をここに記録します。

## プロジェクトの作成

新しいプロジェクトを作成します：

```bash
warder new my-project
cd my-project
```

作成される基本構造：

```text
my-project/
├── package.rl.toml
├── src/
│   └── main.rl
├── tests/
│   └── main_test.rl
├── README.md
└── .gitignore
```

既存ディレクトリをWarderプロジェクトにする場合：

```bash
warder init
```

## パッケージマニフェスト

`package.rl.toml`はパッケージ情報、依存関係、ビルド設定を定義します：

```toml
[package]
name = "my-project"
version = "0.1.0"
description = "My Restrict Language project"
authors = ["Your Name <you@example.com>"]
license = "MIT"
entry = "src/main.rl"
edition = "2025"

[dependencies]
http = "0.8.0"
local-utils = { path = "../local-utils" }
json = { git = "https://github.com/example/json.git", tag = "v1.2.3" }
foreign-math = { wasm = "https://example.com/math.wasm", wit = "https://example.com/math.wit" }

[build]
target = "wasm32"
output_dir = "dist"
optimization = true
```

## 依存関係の管理

レジストリ依存関係：

```bash
warder add http
warder add json@1.0.0
```

ローカル依存関係：

```bash
warder add local-utils --path ../local-utils
```

Git依存関係：

```bash
warder add json@v1.2.3 --git https://github.com/example/json.git
```

外部WASM依存関係：

```bash
warder add foreign-math --wasm https://example.com/math.wasm --wit https://example.com/math.wit
```

依存関係の削除：

```bash
warder remove http
```

## ビルド

```bash
warder build
```

既定の成果物は`dist/<name>-<version>.wat`、`dist/<name>-<version>.wasm`、`dist/<name>-<version>.rgc`です。

`build`は次のフラグを受け付けます：

```bash
warder build --release
warder build --watch
warder build --component
warder build --verify
warder build --repro
```

v0.0.1では、`--release`の最適化、ウォッチモード、WASM Component出力、署名検証、再現可能ビルドは実験的な範囲です。コマンドはその旨を表示し、既定のビルド経路を使います。ターゲットはマニフェストの`[build]`で指定し、ビルドコマンド側のターゲット指定フラグはありません。

## 実行

```bash
warder run
```

プログラム引数を渡す場合：

```bash
warder run -- arg1 arg2
```

`warder run`は先にビルドを実行し、生成されたWASMを`wasmtime`または`wasmer`で実行します。

## テスト

```bash
warder test
```

v0.0.1には専用のテスト宣言構文がないため、`warder test`は`tests/`以下の`.rl`ファイルを型チェック用のスモークテストとして扱います。ファイル名で絞り込む場合：

```bash
warder test main
```

## 公開

```bash
warder publish
warder publish --registry https://example.com
```

v0.0.1の`publish`は事前ビルドとメタデータ検証を行います。レジストリへのアップロードは実験的で、このリリース範囲では実行されません。ローカル評価には生成された`.rgc`を使用します。

## 外部WASMのCage化

外部WASMをCageに包む場合：

```bash
warder wrap module.wasm --name foreign-math --version 0.1.0
warder wrap module.wasm --name foreign-math --version 0.1.0 --wit interface.wit --output foreign-math.rgc
```

生成されるCageはローカル評価向けの実験的な成果物です。

Cageを展開する場合：

```bash
warder unwrap foreign-math.rgc
warder unwrap foreign-math.rgc --output extracted
```

`warder unwrap --component`はフラグとして受け付けますが、WASM Component変換はv0.0.1の実験的な範囲です。

## プロジェクトの健全性チェック

```bash
warder doctor
```

`doctor`はプロジェクトルート、`package.rl.toml`、エントリーポイント、依存関係ロック、基本的な設定問題を確認します。一部の詳細解析はv0.0.1ではスキップされます。

## コマンドリファレンス

| コマンド | 説明 |
|---------|------|
| `warder new <name>` | 新しいプロジェクトを作成 |
| `warder init` | 現在のディレクトリを初期化 |
| `warder add <dep>` | 依存関係を追加 |
| `warder remove <name>` | 依存関係を削除 |
| `warder build` | WAT、WASM、Cageを生成 |
| `warder run` | ビルドして実行 |
| `warder test [filter]` | `tests/`以下を型チェック |
| `warder publish` | 事前ビルドとメタデータ検証 |
| `warder wrap <wasm>` | 外部WASMをCage化 |
| `warder unwrap <cage>` | Cageを展開 |
| `warder doctor` | プロジェクトの健全性を確認 |
