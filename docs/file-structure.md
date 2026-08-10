# Restrict Language 推奨ファイル構造

> 注: この文書は package/warder 構成の設計ノートです。コード例は現行の言語仕様に合わせていますが、公開ドキュメントの正本ではありません。

## 基本構造

```
my-project/
├── warder.toml          # リポジトリルートの唯一のWarder設定
├── warder.lock          # 外部依存解決用ロックファイル (自動生成)
├── src/                 # ソースコード
│   ├── main.rl         # エントリーポイント
│   ├── lib.rl          # ライブラリエントリー (オプション)
│   └── lib/            # モジュール
│       ├── math.rl     # 数学関数
│       ├── string.rl   # 文字列処理
│       └── io.rl       # 入出力
├── tests/              # テストファイル
│   ├── math_test.rl
│   └── string_test.rl
├── examples/           # サンプルコード
│   └── hello.rl
├── dist/              # ビルド出力 (自動生成)
│   ├── my-project-0.1.0.wasm
│   └── my-project-0.1.0.rgc
└── .restrict-cache/   # ビルドキャッシュ (自動生成)
```

`warder.toml`はリポジトリに1つだけ置きます。現行実装は公開可能な
ルートパッケージ1つと、ルート`[dependencies]`から選択する直接ローカル
ソースパッケージを扱います。ローカルパッケージに必要なのは
`src/lib.rl`だけで、leaf manifestやleaf lockは置きません。
`warder.lock`は決定的に再構築できない外部依存解決だけを固定します。
現在はそのresolverがfail closedするため、ローカルのみのlockには解決済み
package entryがありません。

将来の複数公開パッケージworkspaceも同じroot `warder.toml`を拡張します。
別のworkspace設定ファイルやmember manifestは導入せず、公開identityやABIを
ディレクトリ配置から導出しません。`dist/*.component.toml`は生成成果物の
metadataであり、追加のプロジェクト設定ではありません。

## モジュールシステム

### 1. モジュール定義

**src/lib/math.rl**
```restrict
// 関数をエクスポート
pub fun add: (x: Int32, y: Int32) -> Int32 = {
    x + y
}

pub fun multiply: (x: Int32, y: Int32) -> Int32 = {
    x * y
}

// プライベート関数（エクスポートなし）
fun helper: (x: Int32) -> Int32 = {
    x * 2
}
```

### 2. モジュールのインポート

**src/main.rl**
```restrict
// 特定の関数のみインポート
import lib.math.{add, multiply}

// モジュール全体をインポート
import lib.string.*

// エイリアス付きインポート (将来実装予定)
// import lib.io as io

fun main: () = {
    val result = (10, 20) add
    result |> print_int
}
```

## 推奨ディレクトリ構造

### 小規模プロジェクト
```
simple-app/
├── warder.toml
├── src/
│   └── main.rl      # すべてのコードを1ファイルに
└── tests/
    └── main_test.rl
```

### 中規模プロジェクト
```
medium-app/
├── warder.toml
├── src/
│   ├── main.rl
│   └── lib/
│       ├── models.rl    # データモデル
│       ├── utils.rl     # ユーティリティ関数
│       └── api.rl       # API関連
├── tests/
└── examples/
```

### 大規模プロジェクト
```
large-app/
├── warder.toml
├── src/
│   ├── main.rl
│   └── lib/
│       ├── core/        # コア機能
│       │   ├── types.rl
│       │   └── errors.rl
│       ├── domain/      # ドメインロジック
│       │   ├── user.rl
│       │   └── product.rl
│       ├── infra/       # インフラストラクチャ
│       │   ├── db.rl
│       │   └── http.rl
│       └── ui/          # UI関連
│           ├── components.rl
│           └── views.rl
├── tests/
├── examples/
└── docs/
```

## ファイル命名規則

1. **ファイル名**: `snake_case.rl`
2. **モジュールパス**: ドット区切り (`lib.math.utils`)
3. **テストファイル**: `*_test.rl`
4. **エントリーポイント**: `main.rl` または `lib.rl`

## ベストプラクティス

### 1. モジュールの責務分離
```restrict
// ❌ 悪い例: 1つのファイルに複数の責務
// src/lib/utils.rl
pub fun add: (x: Int32, y: Int32) -> Int32 = { ... }
pub fun parse_json: (input: String) -> Json = { ... }
pub fun connect_db: () -> DbConnection = { ... }

// ✅ 良い例: 責務ごとにファイルを分ける
// src/lib/math.rl
pub fun add: (x: Int32, y: Int32) -> Int32 = { ... }

// src/lib/json.rl
pub fun parse: (input: String) -> Json = { ... }

// src/lib/db.rl
pub fun connect: () -> DbConnection = { ... }
```

### 2. 循環依存の回避
```restrict
// ❌ 悪い例: A → B → A の循環
// src/lib/a.rl
import lib.b.{foo}

// src/lib/b.rl
import lib.a.{bar}  // 循環依存！

// ✅ 良い例: 共通モジュールを作る
// src/lib/common.rl
pub fun shared: () = { ... }

// src/lib/a.rl
import lib.common.{shared}

// src/lib/b.rl
import lib.common.{shared}
```

### 3. 公開APIの明確化
```restrict
// src/lib/user.rl

// 公開API（pub付き）
pub record User {
    id: Int32
    name: String
}

pub fun create_user: (name: String) -> User = {
    val id = () generate_id
    User { id: id, name: name }
}

// 内部実装（exportなし）
fun generate_id: () -> Int32 = {
    // 実装詳細は非公開
    42
}
```

## warderとの統合

### warder.toml の設定
```toml
[package]
name = "my-app"
version = "0.1.0"
entry = "src/main.rl"    # エントリーポイント
edition = "2025"

[dependencies]
my_lib = { path = "../my-lib" }  # 直接ローカルソース依存

[build]
target = "wasm32"
output = "dist/"
```

### ビルドコマンド
```bash
# プロジェクトのビルド
warder build

# テストの実行
warder test

# 関数名でテストを絞り込み
warder test math

# リリースビルド
warder build --release
```

## まとめ

この構造により：
1. **モジュール性**: 機能ごとにファイルを分離
2. **再利用性**: exportによる公開APIの明確化
3. **保守性**: 責務の分離と適切な命名
4. **拡張性**: プロジェクト規模に応じた構造

が実現できます。
