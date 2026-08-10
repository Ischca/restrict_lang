# Warderパッケージマネージャー

WarderはRestrict Languageのプロジェクト作成、依存関係管理、ビルド、実行、テストをまとめるツールです。v0.0.1で公開対象として扱うコマンドは、`new`、`init`、`add`、`remove`、`build`、`run`、`test`、`publish`、`wrap`、`unwrap`、`doctor`です。

## 主要概念

### Cageフォーマット（.rgc）

CageはWarderのパッケージ成果物です。`warder build`は既定で`dist/<name>-<version>.wat`、`dist/<name>-<version>.wasm`、`dist/<name>-<version>.rgc`を生成します。

### ルートロックファイル（warder.lock）

`warder.lock`はWarderがリポジトリルートに生成する唯一のロックファイルです。決定的に再構築できない外部依存関係の解決結果だけを記録します。直接ローカルソースパッケージはロック対象ではなく、現在のローカルのみのlockに解決済みパッケージエントリはありません。

## プロジェクトの作成

新しいプロジェクトを作成します：

```bash
warder new my-project
cd my-project
```

作成される基本構造：

```text
my-project/
├── warder.toml
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

## リポジトリ設定

`warder.toml`はWarder管理リポジトリで人が編集する唯一の設定ファイルです。`warder new`と`warder init`は次の最小形を生成します：

```toml
manifest-version = 1

[package]
name = "my-project"
version = "0.1.0"
```

規約値としてentryは`src/main.rl`、editionは`2025`、buildはtarget
`wasm32`・最適化有効・出力先`dist/`が適用され、空のdependenciesは省略
されます。`package.entry`、`package.edition`、`build`は規約値を変更する
場合だけ記述します。`description`と`authors`は必要に応じて追加できる
任意のメタデータです。

ローカル依存を追加した場合だけ、次のテーブルが加わります：

```toml
[dependencies]
local_utils = { path = "../local-utils" }
```

現行実装が公開可能なパッケージはルートの1つです。直接ローカルソースパッケージはこのルート設定から選択し、必要なのは`src/lib.rl`だけです。leaf manifestやleaf lockは持ちません。将来の複数公開パッケージworkspaceも同じルートファイルを拡張し、公開identityをディレクトリ配置から導出しません。

## 依存関係の管理

ローカル依存関係：

```bash
warder add local_utils --path ../local-utils
```

レジストリ、Git、外部WASM、推移的依存関係の解決は現在fail closedします。`warder build`はroot `warder.lock`を生成しますが、直接ローカルソースはlock entryにしません。生成された`warder.lock`は手で編集せず、リポジトリへコミットします。

依存関係の削除：

```bash
warder remove local_utils
```

## ビルド

```bash
warder build
```

`[component]`がない場合の既定成果物は`dist/<name>-<version>.wat`、`dist/<name>-<version>.wasm`、`dist/<name>-<version>.rgc`です。すべてのコマンドで`--project <dir>`を指定できます。

`build`は次のフラグを受け付けます：

```bash
warder build --release
warder build --watch
warder build --component
warder build --core
warder build --verify
warder build --repro
```

`--watch`はルート設定、プロジェクトの`.rl`、直接ローカル依存関係の`.rl`を監視してフルビルドします。`[component]`がある場合、通常の`warder build`はComponent成果物を生成します。`--component`はその選択を要求し、`--core`はCore成果物を強制します。現行実装ではsource-publicと`[component]`の二重gateが必要です。

Componentビルドは、ロック更新とともに次の一式を復旧可能なトランザクションで公開します：

```text
dist/<name>-<version>.component.wasm
dist/<name>-<version>.component.wit
dist/<name>-<version>.component.toml
```

`.component.toml`は意味的なinterface hashとComponent/WITの内容hashを分離して記録する生成成果物であり、プロジェクト設定ではありません。再ビルドが失敗した場合は以前の完全な一式を維持します。schema-v2はルート宣言に限定され、resource、値だけをcaptureするonce-callable、明示的なgeneric関数の具体化を追加します。

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

`warder test`は、名前が`test_`で始まるか`_test`で終わるpublicな`() -> Boolean`関数をWASI Preview 1 Core Wasmへコンパイルして実行します。`true`は成功、`false`やtrapは失敗です。filterは関数名に適用されます：

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

生成されるCageはローカル評価向けの実験的な成果物です。`--wit`は指定されたWITテキストをCageへ添付するだけで、WITの生成・解析・型検証、Core moduleとの整合確認、Component化は行いません。これはRestrictパッケージからComponent一式を生成・検証する`warder build --component`とは別機能です。

Cageを展開する場合：

```bash
warder unwrap foreign-math.rgc
warder unwrap foreign-math.rgc --output extracted
```

`warder unwrap --component`は受け付けますが、Cage展開時のWASM Component変換は行いません。

## プロジェクトの健全性チェック

```bash
warder doctor
```

`doctor`はリポジトリルート、`warder.toml`、エントリーポイント、`warder.lock`、基本的な設定問題を確認します。一部の詳細解析はv0.0.1ではスキップされます。

## コマンドリファレンス

| コマンド | 説明 |
|---------|------|
| `warder new <name>` | 新しいプロジェクトを作成 |
| `warder init` | 現在のディレクトリを初期化 |
| `warder add <alias> --path <dir>` | 直接ローカル依存関係を追加 |
| `warder remove <name>` | 依存関係を削除 |
| `warder build` | WAT、WASM、Cageを生成 |
| `warder run` | ビルドして実行 |
| `warder test [filter]` | `tests/`以下のBooleanテストを実行 |
| `warder publish` | 事前ビルドとメタデータ検証 |
| `warder wrap <wasm>` | 外部WASMをCage化 |
| `warder unwrap <cage>` | Cageを展開 |
| `warder doctor` | プロジェクトの健全性を確認 |
