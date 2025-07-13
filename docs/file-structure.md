# Restrict Language 推奨ファイル構造

## 基本構造

```
my-project/
├── package.rl.toml      # プロジェクトマニフェスト
├── restrict-lock.toml   # 依存関係ロックファイル (自動生成)
├── src/                 # ソースコード
│   ├── Main.rl         # エントリーポイント
│   ├── Lib.rl          # ライブラリエントリー (オプション)
│   └── lib/            # モジュール
│       ├── Math.rl     # 数学関数
│       ├── String.rl   # 文字列処理
│       └── IO.rl       # 入出力
├── tests/              # テストファイル
│   ├── MathTest.rl
│   └── StringTest.rl
├── examples/           # サンプルコード
│   └── Hello.rl
├── dist/              # ビルド出力 (自動生成)
│   ├── my-project-0.1.0.wasm
│   └── my-project-0.1.0.rgc
└── .restrict-cache/   # ビルドキャッシュ (自動生成)
```

## モジュールシステム

### 1. モジュール定義

**src/lib/Math.rl**
```restrict
// 関数をエクスポート
export fun add = x:Int32 -> y:Int32 -> Int32 {
    x + y
}

export fun multiply = x:Int32 -> y:Int32 -> Int32 {
    x * y
}

// プライベート関数（エクスポートなし）
fun helper = x:Int32 -> Int32 {
    x * 2
}
```

### 2. モジュールのインポート

**src/Main.rl**
```restrict
// 特定の関数のみインポート
import lib.Math.{add, multiply}

// モジュール全体をインポート
import lib.String.*

// エイリアス付きインポート (将来実装予定)
// import lib.IO as io

fun main = {
    val result = add(10, 20);
    result |> println
}
```

## 推奨ディレクトリ構造

### 小規模プロジェクト
```
simple-app/
├── package.rl.toml
├── src/
│   └── Main.rl      # すべてのコードを1ファイルに
└── tests/
    └── MainTest.rl
```

### 中規模プロジェクト
```
medium-app/
├── package.rl.toml
├── src/
│   ├── Main.rl
│   └── lib/
│       ├── Models.rl    # データモデル
│       ├── Utils.rl     # ユーティリティ関数
│       └── Api.rl       # API関連
├── tests/
└── examples/
```

### 大規模プロジェクト
```
large-app/
├── package.rl.toml
├── src/
│   ├── Main.rl
│   └── lib/
│       ├── core/        # コア機能
│       │   ├── Types.rl
│       │   └── Errors.rl
│       ├── domain/      # ドメインロジック
│       │   ├── User.rl
│       │   └── Product.rl
│       ├── infra/       # インフラストラクチャ
│       │   ├── Db.rl
│       │   └── Http.rl
│       └── ui/          # UI関連
│           ├── Components.rl
│           └── Views.rl
├── tests/
├── examples/
└── docs/
```

## ファイル命名規則

1. **ファイル名**: `CamelCase.rl`
2. **モジュールパス**: ドット区切り (`lib.Math.Utils`)
3. **テストファイル**: `*Test.rl`
4. **エントリーポイント**: `Main.rl` または `Lib.rl`

## ベストプラクティス

### 1. モジュールの責務分離
```restrict
// ❌ 悪い例: 1つのファイルに複数の責務
// src/lib/Utils.rl
export fun add = ...
export fun parse_json = ...
export fun connect_db = ...

// ✅ 良い例: 責務ごとにファイルを分ける
// src/lib/Math.rl
export fun add = ...

// src/lib/Json.rl
export fun parse = ...

// src/lib/Db.rl
export fun connect = ...
```

### 2. 循環依存の回避
```restrict
// ❌ 悪い例: A → B → A の循環
// src/lib/A.rl
import lib.B.{foo}

// src/lib/B.rl
import lib.A.{bar}  // 循環依存！

// ✅ 良い例: 共通モジュールを作る
// src/lib/Common.rl
export fun shared = ...

// src/lib/A.rl
import lib.Common.{shared}

// src/lib/B.rl
import lib.Common.{shared}
```

### 3. 公開APIの明確化
```restrict
// src/lib/User.rl

// 公開API（export付き）
export record User {
    id: Int32,
    name: String,
}

export fun create_user = name:String -> User {
    User { id: generate_id(), name: name }
}

// 内部実装（exportなし）
fun generate_id = Unit -> Int32 {
    // 実装詳細は非公開
    42
}
```

## warderとの統合

### package.rl.toml の設定
```toml
[package]
name = "my-app"
version = "0.1.0"
entry = "src/Main.rl"    # エントリーポイント
edition = "2025"

[dependencies]
restrict-std = "0.2.0"   # 標準ライブラリ
my-lib = { path = "../my-lib" }  # ローカル依存

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

# 特定モジュールのテスト
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