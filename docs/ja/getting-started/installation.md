# インストール

このガイドでは、Restrict Languageのインストールと開発環境のセットアップ方法を説明します。

## システム要件

Restrict Languageは以下のプラットフォームをサポートしています：

- **macOS** (x86_64、ARM64)
- **Linux** (x86_64、ARM64)
- **Windows** (x86_64、WSL2の使用を推奨)

### 前提条件

- **Rust** 1.70以降（ソースからビルドする場合）
- **Git**（バージョン管理用）
- **テキストエディタ**（VS Codeを推奨）

## インストール方法

### 方法1: miseを使用（推奨）

[mise](https://mise.jdx.dev/)は、Restrict Languageを簡単にインストール・管理できる多言語ランタイムマネージャーです。

```bash
# miseをまだインストールしていない場合
curl https://mise.run | sh

# シェルにmiseを追加
echo 'eval "$(~/.local/bin/mise activate bash)"' >> ~/.bashrc
# zshユーザーの場合:
# echo 'eval "$(~/.local/bin/mise activate zsh)"' >> ~/.zshrc

# Restrict Languageをインストール
mise use restrict_lang@latest
```

### 方法2: ソースからビルド

公式リポジトリからクローンしてビルド：

```bash
# リポジトリをクローン
git clone https://github.com/restrict-lang/restrict_lang.git
cd restrict_lang

# miseでビルド
mise run build-release

# またはcargoで直接ビルド
cargo build --release
cd warder && cargo build --release
```

ビルド後、バイナリをPATHに追加：

```bash
# ~/.bashrcまたは~/.zshrcに追加
export PATH="$PATH:$HOME/restrict_lang/target/release"
```

### 方法3: ビルド済みバイナリ

リリースページからビルド済みバイナリをダウンロード：

```bash
# macOS (Intel)
curl -L https://github.com/restrict-lang/restrict_lang/releases/latest/download/restrict_lang-darwin-x86_64.tar.gz | tar xz

# macOS (Apple Silicon)
curl -L https://github.com/restrict-lang/restrict_lang/releases/latest/download/restrict_lang-darwin-aarch64.tar.gz | tar xz

# Linux (x86_64)
curl -L https://github.com/restrict-lang/restrict_lang/releases/latest/download/restrict_lang-linux-x86_64.tar.gz | tar xz

# バイナリをPATH内の場所に移動
sudo mv restrict_lang warder /usr/local/bin/
```

## インストールの確認

コンパイラとパッケージマネージャーが正しくインストールされていることを確認：

```bash
# Restrict Languageコンパイラを確認
restrict_lang --version
# 期待される出力: restrict_lang 0.1.0

# Warderパッケージマネージャーを確認
warder --version
# 期待される出力: warder 0.1.0
```

## WebAssemblyランタイムのインストール

Restrict LanguageはWebAssemblyにコンパイルされるため、プログラムを実行するにはWASMランタイムが必要です：

### オプション1: Wasmtime（推奨）

```bash
# wasmtimeをインストール
curl https://wasmtime.dev/install.sh -sSf | bash
```

### オプション2: Wasmer

```bash
# wasmerをインストール
curl https://get.wasmer.io -sSfL | sh
```

## IDE設定

### VS Code拡張機能

最高の開発体験のために、公式VS Code拡張機能をインストールしてください：

1. VS Codeを開く
2. 拡張機能に移動（macOSではCmd+Shift+X、Windows/LinuxではCtrl+Shift+X）
3. "Restrict Language"を検索
4. インストールをクリック

拡張機能が提供する機能：
- シンタックスハイライト
- 自動補完
- エラーチェック
- 定義へジャンプ
- 参照の検索
- 保存時フォーマット

### その他のエディタ

他のエディタでは、Language Server Protocol（LSP）を使用できます：

```bash
# 言語サーバーを起動
restrict_lang lsp
```

デフォルトポート（7777）で言語サーバーに接続するようエディタを設定してください。

## 開発ツール

追加の開発ツールをインストール：

```bash
# すべての開発依存関係をインストール
mise run setup

# または手動でインストール
cargo install cargo-watch cargo-audit cargo-tarpaulin
```

## 設定

グローバル設定ファイルを作成：

```bash
mkdir -p ~/.config/restrict_lang
cat > ~/.config/restrict_lang/config.toml << EOF
[compiler]
optimization_level = 2
target = "wasm32-wasi"

[warder]
registry = "https://wardhub.io"
cache_dir = "~/.cache/warder"

[editor]
format_on_save = true
lint_on_save = true
EOF
```

## トラブルシューティング

### よくある問題

**コマンドが見つからない**
- バイナリがPATHに含まれていることを確認
- ターミナルを再起動するか`source ~/.bashrc`を実行

**アクセス拒否**
- バイナリを実行可能にする: `chmod +x restrict_lang warder`
- システムディレクトリに移動する際は`sudo`を使用

**ビルド失敗**
- Rustが最新であることを確認: `rustup update`
- システム依存関係がインストールされていることを確認

### ヘルプを得る

- [FAQ](../appendix/faq.md)を確認
- [Discordコミュニティ](https://discord.gg/restrict-lang)に参加
- [GitHub](https://github.com/restrict-lang/restrict_lang/issues)で問題を報告

## 次のステップ

Restrict Languageのインストールが完了したら、次のことができます：

- [最初のプログラムを書く](./hello-world.md)
- [Warderパッケージマネージャーについて学ぶ](./warder.md)
- [言語ガイドを探索する](../guide/syntax.md)