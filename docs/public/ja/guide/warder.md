# Warderパッケージマネージャー

WarderはRestrict Languageのプロジェクト作成、依存関係管理、ビルド、実行、テストをまとめるツールです。v0.0.1で公開対象として扱うコマンドは、`new`、`init`、`add`、`remove`、`build`、`run`、`test`、`publish`、`wrap`、`unwrap`、`doctor`です。

## 主要概念

### Cageフォーマット（.rgc）

CageはWarderのパッケージ成果物です。`warder build`は既定で`dist/<name>-<version>.wat`、`dist/<name>-<version>.wasm`、`dist/<name>-<version>.rgc`を生成します。

### ヴォールト（restrict-lock.toml）

`restrict-lock.toml`は依存関係のロックファイルです。直接ローカル依存関係について、依存先マニフェストのバージョンと、マニフェストおよびRestrictソースから決定的に計算したSHA-256を記録します。

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
local_utils = { path = "../local-utils" }

[build]
target = "wasm32"
output = "dist/"
optimize = true
```

## 依存関係の管理

v0.0.1でビルドできる依存関係は、直接参照するローカルパス依存関係だけです。`[dependencies]`のキーがRestrictソースで使う名前空間になります。この別名は予約語ではない単一のRestrict識別子である必要があります。`local-utils`は暗黙に変換されないため、`local_utils`のように記述してください。`std`は予約済みです。

ローカル依存関係を追加します：

```bash
warder add local_utils --path ../local-utils
```

依存先には`package.rl.toml`と`src/lib.rl`が必要です：

```text
local-utils/
├── package.rl.toml
└── src/
    ├── lib.rl
    └── numbers.rl
```

依存先の`src/lib.rl`は、たとえば次のように公開関数を定義できます：

```restrict
pub fun score: () -> Int32 = {
    42
}
```

アプリケーション側のインポートとファイルの対応は次のとおりです：

| ソースのインポート | 依存先ファイル |
|--------------------|----------------|
| `import local_utils.{score}` | `../local-utils/src/lib.rl` |
| `import local_utils` | `../local-utils/src/lib.rl` |
| `import local_utils.numbers.{double}` | `../local-utils/src/numbers.rl` |

依存パッケージ内の修飾なしインポートは、その依存パッケージ内で解決されます。たとえば`local-utils/src/lib.rl`内の`import numbers.{double}`は、アプリケーション側ではなく同じパッケージの`src/numbers.rl`を参照します。

これはマニフェストとコンパイラ間の名前空間バインドです。ソース構文としての`import ... as`やre-exportは引き続き未対応です。

レジストリ、Git、外部WASM、推移的依存関係はまだ解決しません。`warder add`、`warder build`、`warder test`はこれらを明示的にエラーにし、見せかけのロックエントリを生成しません。外部WASMのローカル評価には、別機能の`warder wrap`を使用してください。

Warderはアプリケーションと各依存パッケージのソースを不変スナップショットへコピーしてからコンパイルします。アプリケーション、依存先、出力先のルートが重なる構成は拒否します。同じプロジェクトの並行ビルドは直列化され、WAT、WASM、Cage、ロックファイルは復旧可能な一括更新として公開されます。コンパイル失敗時は以前の成果物一式を維持します。

依存関係の削除：

```bash
warder remove local_utils
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

v0.0.1には専用のテスト宣言構文がないため、`warder test`は`tests/`以下の`.rl`ファイルを型チェック用のスモークテストとして扱います。`warder build`と同じ直接ローカル依存関係のルートを使用するため、テストからも同じパッケージをインポートできます。コンパイラのフォールバック解決はWarderを起動したサブディレクトリではなくプロジェクトルートに固定されます。ファイル名で絞り込む場合：

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

`doctor`はプロジェクトルート、`package.rl.toml`、エントリーポイント、依存関係ロック、基本的な設定問題を確認します。直接ローカル依存関係については、ロックの欠落、形式不正、ソース変更による古さも検出します。一部の詳細解析はv0.0.1ではスキップされます。

## コマンドリファレンス

| コマンド | 説明 |
|---------|------|
| `warder new <name>` | 新しいプロジェクトを作成 |
| `warder init` | 現在のディレクトリを初期化 |
| `warder add <alias> --path <dir>` | 直接ローカル依存関係を追加 |
| `warder remove <name>` | 依存関係を削除 |
| `warder build` | WAT、WASM、Cageを生成 |
| `warder run` | ビルドして実行 |
| `warder test [filter]` | `tests/`以下を型チェック |
| `warder publish` | 事前ビルドとメタデータ検証 |
| `warder wrap <wasm>` | 外部WASMをCage化 |
| `warder unwrap <cage>` | Cageを展開 |
| `warder doctor` | プロジェクトの健全性を確認 |
