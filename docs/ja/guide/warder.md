# Warderパッケージマネージャー

WarderはRestrict Languageの公式パッケージマネージャーで、依存関係の管理、プロジェクトのスキャフォールディング、コードの配布を簡素化するように設計されています。守護者や管理人を意味する名前の通り、Warderはプロジェクトの依存関係を安全な「ヴォールト」で管理し、Restrictコンパイラとのシームレスな統合を提供します。

## 主要概念

### Cageフォーマット（.rgc）

CageフォーマットはWarderのパッケージ配布フォーマットで、npmのtarballやRustのcrateに似ています。Cageには以下が含まれます：
- コンパイル済みWebAssemblyモジュール
- ソースコード（オプション）
- パッケージメタデータ
- 依存関係

### ヴォールト（restrict-lock.toml）

ヴォールトはプロジェクトの依存関係ロックファイルで、異なる環境間での再現可能なビルドを保証します。すべての依存関係の正確なバージョンとチェックサムを記録します。

### WardHub

WardHubはRestrictパッケージの中央レジストリで、開発者がパッケージを公開・発見できる場所です。

## インストール

WarderはRestrict Languageコンパイラにバンドルされています。インストールを確認：

```bash
warder --version
```

## はじめに

### 新しいプロジェクトの作成

```bash
warder new my-project
cd my-project
```

これにより、以下の構造で新しいRestrictプロジェクトが作成されます：

```
my-project/
├── package.rl.toml     # パッケージマニフェスト
├── src/
│   └── main.rl         # エントリーポイント
├── tests/
│   └── main_test.rl    # テストの例
└── .gitignore
```

### 既存プロジェクトの初期化

```bash
warder init
```

現在のディレクトリに`package.rl.toml`ファイルを作成します。

## パッケージマニフェスト（package.rl.toml）

パッケージマニフェストは、プロジェクトのメタデータと依存関係を定義します：

```toml
[package]
name = "my-awesome-lib"
version = "0.1.0"
authors = ["あなたの名前 <you@example.com>"]
description = "パッケージの簡単な説明"
license = "MIT"
repository = "https://github.com/username/my-awesome-lib"
keywords = ["web", "async", "http"]

[dependencies]
# 通常の依存関係
http = "0.8.0"
json = { version = "1.0", features = ["streaming"] }
utils = { git = "https://github.com/user/utils", branch = "main" }

[dev-dependencies]
# 開発/テスト用の依存関係
test-framework = "0.5.0"

[build-dependencies]
# ビルドスクリプト用の依存関係
wasm-bindgen = "0.2"

[target.'cfg(wasm)'.dependencies]
# プラットフォーム固有の依存関係
wasm-specific = "0.1.0"

[features]
default = ["std"]
std = []
no-std = []

[[bin]]
name = "my-app"
path = "src/bin/main.rl"

[lib]
name = "my_lib"
path = "src/lib.rl"
```

## 依存関係の管理

### 依存関係の追加

WardHubから依存関係を追加：

```bash
warder add http
warder add json@1.0.0
warder add async-runtime --features runtime,macros
```

開発用依存関係を追加：

```bash
warder add --dev test-framework
```

Gitから追加：

```bash
warder add --git https://github.com/user/package
```

### 依存関係の更新

すべての依存関係を最新の互換バージョンに更新：

```bash
warder update
```

特定の依存関係を更新：

```bash
warder update http
```

### 依存関係の削除

```bash
warder remove http
```

### 依存関係の一覧表示

プロジェクトの依存関係ツリーを表示：

```bash
warder tree
```

出力：
```
my-project v0.1.0
├── http v0.8.0
│   ├── async-io v1.3.0
│   └── url v2.2.0
├── json v1.0.0
└── utils v0.2.0 (git+https://github.com/user/utils)
```

## プロジェクトのビルド

### 開発ビルド

```bash
warder build
```

これは、高速なコンパイルのために最適化を無効にしたデバッグモードでプロジェクトをコンパイルします。

### リリースビルド

```bash
warder build --release
```

本番環境向けに最適化されたWebAssemblyモジュールを生成します。

### 特定のターゲットのビルド

```bash
warder build --target wasm32-wasi
warder build --target wasm32-unknown-unknown
```

## プロジェクトの実行

### メインバイナリの実行

```bash
warder run
```

### 特定のバイナリの実行

```bash
warder run --bin my-app
```

### 引数付きで実行

```bash
warder run -- arg1 arg2
```

## テスト

### テストの実行

```bash
warder test
```

### 特定のテストの実行

```bash
warder test test_name
warder test --test integration_test
```

### テストカバレッジ

```bash
warder test --coverage
```

## パッケージの公開

### 公開の準備

1. `package.rl.toml`が完全であることを確認
2. README.mdファイルを追加
3. 適切なライセンスを選択
4. パッケージを徹底的にテスト

### WardHubへの公開

```bash
warder login
warder publish
```

公開前に、Warderは以下を行います：
- パッケージマニフェストの検証
- テストの実行
- 一般的な問題のチェック
- パッケージのビルド

### バージョニング

セマンティックバージョニングに従う：
- MAJOR: 破壊的変更
- MINOR: 新機能（後方互換性あり）
- PATCH: バグ修正

バージョン更新：

```bash
warder version patch  # 0.1.0 -> 0.1.1
warder version minor  # 0.1.1 -> 0.2.0
warder version major  # 0.2.0 -> 1.0.0
```

## ワークスペース

マルチパッケージプロジェクトには、ワークスペースを使用：

```toml
# workspace.rl.toml
[workspace]
members = [
    "packages/core",
    "packages/cli",
    "packages/web"
]

[workspace.dependencies]
common = { path = "packages/common" }
```

## 高度な機能

### カスタムレジストリ

代替レジストリの設定：

```toml
[registries]
my-registry = { index = "https://my-registry.com/index" }

[dependencies]
private-package = { version = "1.0", registry = "my-registry" }
```

### 依存関係のベンダリング

すべての依存関係をローカルにダウンロード：

```bash
warder vendor
```

### Cageの検査

Cageファイルを調べる：

```bash
warder cage inspect package-1.0.0.rgc
```

Cageを展開：

```bash
warder cage extract package-1.0.0.rgc
```

### ビルドスクリプト

`build.rl`にビルドスクリプトを追加：

```restrict
fn main() {
    // コード生成、リソースのコンパイルなど
    generateBindings();
}
```

`package.rl.toml`で設定：

```toml
[package]
build = "build.rl"
```

## 設定

### グローバル設定

`~/.warder/config.toml`に配置：

```toml
[registry]
default = "https://wardhub.io"
token = "your-auth-token"

[build]
jobs = 4
target-dir = "target"

[net]
offline = false
timeout = 30
```

### プロジェクト設定

`.warder/config.toml`でグローバル設定を上書き：

```toml
[build]
opt-level = 3
debug = false
```

## トラブルシューティング

### プロジェクトの健全性チェック

```bash
warder doctor
```

このコマンドは以下をチェックします：
- パッケージマニフェストの妥当性
- 依存関係の競合
- 不足ファイル
- 設定の問題

### ビルド成果物のクリーン

```bash
warder clean
```

### 詳細な出力

```bash
warder build --verbose
```

### オフラインモード

ネットワークアクセスなしで作業：

```bash
warder build --offline
```

## CI/CDとの統合

### GitHub Actions

```yaml
name: ビルドとテスト
on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - uses: restrict-lang/setup-restrict@v1
      - run: warder test
      - run: warder build --release
```

### Docker

```dockerfile
FROM restrict-lang/restrict:latest
WORKDIR /app
COPY . .
RUN warder build --release
CMD ["warder", "run", "--release"]
```

## ベストプラクティス

1. **常にrestrict-lock.tomlをコミット** - 再現可能なビルドを保証
2. **セマンティックバージョニングを使用** - 依存関係の解決を予測可能に
3. **依存関係を最小限に** - より小さなバイナリと攻撃面
4. **公開前にテスト** - `warder test`と`warder package`を実行
5. **パッケージをドキュメント化** - 例とAPIドキュメントを含める

## よく使うコマンドリファレンス

| コマンド | 説明 |
|---------|------|
| `warder new <name>` | 新しいプロジェクトを作成 |
| `warder init` | 既存プロジェクトを初期化 |
| `warder build` | プロジェクトをコンパイル |
| `warder run` | ビルドして実行 |
| `warder test` | テストを実行 |
| `warder add <pkg>` | 依存関係を追加 |
| `warder remove <pkg>` | 依存関係を削除 |
| `warder update` | 依存関係を更新 |
| `warder publish` | WardHubに公開 |
| `warder search <query>` | パッケージを検索 |
| `warder doc` | ドキュメントを生成 |
| `warder clean` | ビルド成果物を削除 |
| `warder tree` | 依存関係ツリーを表示 |
| `warder doctor` | プロジェクトの健全性をチェック |

## まとめ

Warderは、プロジェクトの作成からデプロイまで、Restrict Languageの完全なパッケージ管理ソリューションを提供します。コンパイラとの統合、セキュリティへの焦点、WebAssemblyファーストのアプローチにより、現代のWeb開発に理想的です。