# mise使用ガイド

## インストール
```bash
# miseをインストール（まだの場合）
curl https://mise.run | sh

# プロジェクトのセットアップ
mise install
mise run setup
```

## よく使うコマンド

### 開発中
```bash
# 開発サーバー（自動再コンパイル＆テスト）
mise run dev

# インタラクティブREPL
mise run repl

# ファイル監視モード
mise run watch

# 特定ファイルをデバッグ
mise run debug FILE=test/example.rl
```

### テスト
```bash
# 全テスト実行
mise run test

# 特定のテストのみ
mise run test-one TEST=test_mutable_vars

# プロパティベーステスト
mise run prop-test

# 全ファイルチェック
mise run check
```

### コード品質
```bash
# フォーマット
mise run fmt

# リント
mise run lint

# CI全体を実行
mise run ci
```

### 実行
```bash
# コンパイルして実行
mise run run FILE=test/example.rl

# クイックテスト（式を入力）
mise run quick
```

### その他
```bash
# ドキュメント生成
mise run doc

# クリーンアップ
mise run clean

# 利用可能なタスク一覧
mise tasks
```

## Makefileとの違い

| Makefile | mise |
|----------|------|
| `make test` | `mise run test` |
| `make test-one TEST=foo` | `mise run test-one TEST=foo` |
| `make watch` | `mise run watch` |
| `make clean` | `mise run clean` |

## 便利な機能

1. **依存関係**: `depends`でタスクの依存関係を定義
2. **環境変数**: `.mise.toml`で環境変数を管理
3. **ツール管理**: Rustのバージョンも管理可能
4. **複数コマンド**: 配列で複数のコマンドを順次実行

## 追加カスタマイズ

`.mise.local.toml`を作成して、個人用の設定を追加できます：

```toml
[tasks.my-test]
description = "My custom test"
run = "cargo test my_specific_test"
```