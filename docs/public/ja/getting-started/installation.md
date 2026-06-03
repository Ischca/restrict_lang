# インストール

このガイドでは、Restrict LanguageとWarderをソースからビルドして使う手順を説明します。配布方法やリリース版の識別子はリリースごとの案内に従ってください。

## システム要件

- **Rust**（ソースからビルドする場合）
- **mise**（このリポジトリの開発コマンドで使用）
- **Git**
- **テキストエディタ**（VS Codeなど）

## ソースからビルド

```bash
git clone https://github.com/restrict-lang/restrict_lang.git
cd restrict_lang
mise exec -- cargo build --workspace --release
```

ビルド後、コンパイラとWarderをPATHに追加します：

```bash
export PATH="$PWD/target/release:$PWD/warder/target/release:$PATH"
```

通常のワークスペースビルドでは`target/release`に`restrict_lang`と`warder`が生成されます。Warderを`warder/`配下で個別にビルドした環境では`warder/target/release`もPATHに含めてください。

## インストールの確認

```bash
restrict_lang --version
warder --version
restrict_lang --help
warder --help
```

## WebAssemblyランタイム

`warder run`で生成済みWASMを実行するには、`wasmtime`または`wasmer`が必要です。

```bash
curl https://wasmtime.dev/install.sh -sSf | bash
```

## 基本コマンド

コンパイラを直接使う場合：

```bash
restrict_lang hello.rl hello.wat
restrict_lang --check hello.rl
```

Warderでプロジェクトを扱う場合：

```bash
warder new my-project
cd my-project
warder build
warder run
warder test
warder doctor
```

`warder build`の既定出力は`dist/<name>-<version>.wat`、`dist/<name>-<version>.wasm`、`dist/<name>-<version>.rgc`です。

## 開発用コマンド

このリポジトリで開発する場合、Cargoコマンドは`mise exec --`経由で実行します：

```bash
mise exec -- cargo build
mise exec -- cargo test
mise exec -- cargo run --bin restrict_lang -- hello.rl hello.wat
```

用意されているmiseタスクも利用できます：

```bash
mise run build
mise run test
mise run fmt
mise run lint
mise run ci
```

## IDE設定

Language Server Protocol（LSP）対応エディタでは、次のコマンドをstdioの言語サーバーとして設定できます：

```bash
restrict_lang --lsp
```

## トラブルシューティング

**コマンドが見つからない**

PATHにビルド成果物のディレクトリが含まれていることを確認してください：

```bash
export PATH="$PWD/target/release:$PWD/warder/target/release:$PATH"
which restrict_lang
which warder
```

**ビルドに失敗する**

Rustとmiseが利用できることを確認し、ワークスペースルートでビルドしてください：

```bash
rustc --version
mise --version
mise exec -- cargo build --workspace --release
```

**WASMを実行できない**

`warder run`は`wasmtime`または`wasmer`を探します。どちらかをインストールしてPATHに含めてください。

## 次のステップ

- [最初のプログラムを書く](./hello-world.md)
- [Warderパッケージマネージャーについて学ぶ](../guide/warder.md)
- [言語ガイドを探索する](../guide/syntax.md)
