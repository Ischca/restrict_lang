# Restrict Language 開発環境セットアップ

現在は開発段階のため、ソースからビルドして使用します。

## 必要なもの

- Rust (最新の stable)
- Git

## セットアップ手順

### 1. リポジトリのクローン

```bash
git clone https://github.com/restrict-lang/restrict_lang.git
cd restrict_lang
```

### 2. ビルド

```bash
# Restrict Language コンパイラをビルド
cargo build --release

# Warder パッケージマネージャをビルド
cd warder
cargo build --release
cd ..
```

### 3. PATHの設定

```bash
# 一時的に使用する場合
export PATH="$PWD/target/release:$PWD/warder/target/release:$PATH"

# 永続的に使用する場合（~/.bashrc or ~/.zshrc に追加）
export PATH="$HOME/workspace/ischca/restrict_lang/target/release:$HOME/workspace/ischca/restrict_lang/warder/target/release:$PATH"
```

### 4. 動作確認

```bash
# バージョン確認
restrict_lang --version
warder --version
```

## 開発用コマンド

```bash
# 開発ビルド（デバッグ情報付き）
cargo build

# テスト実行
cargo test

# ドキュメント生成
cargo doc --open

# フォーマット
cargo fmt

# Lint
cargo clippy
```

## VS Code 設定

1. Rust Analyzer 拡張機能をインストール
2. ワークスペース設定:

`.vscode/settings.json`:
```json
{
    "rust-analyzer.linkedProjects": [
        "./Cargo.toml",
        "./warder/Cargo.toml"
    ]
}
```

## プロジェクトの作成と実行

```bash
# 新しいプロジェクトを作成
warder new hello-world
cd hello-world

# 実行
warder run

# ビルド
warder build

# テスト
warder test
```

## トラブルシューティング

### `command not found` エラー

```bash
# フルパスで実行
~/workspace/ischca/restrict_lang/target/release/warder new test-project

# または alias を設定
alias warder="$HOME/workspace/ischca/restrict_lang/warder/target/release/warder"
alias restrict_lang="$HOME/workspace/ischca/restrict_lang/target/release/restrict_lang"
```

### ビルドエラー

```bash
# 依存関係を更新
cargo update

# クリーンビルド
cargo clean
cargo build --release
```

## 開発に参加する

1. フォークを作成
2. フィーチャーブランチを作成
3. コードを変更
4. テストを実行
5. プルリクエストを送信

```bash
# 例
git checkout -b feature/my-feature
# ... コードを変更 ...
cargo test
git commit -m "Add my feature"
git push origin feature/my-feature
```

## 今後の予定

- [ ] バイナリリリース
- [ ] Homebrew 対応
- [ ] インストーラー
- [ ] IDE プラグイン
- [ ] ドキュメントの充実

現在は開発段階ですが、基本的な機能は動作します！