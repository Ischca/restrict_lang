# Type Inference System Design: Constraint-Based Bidirectional Approach

## ステータス: 設計完了・実装待ち

## 背景

### 現状の問題

現在の型推論は `check_function_call_with_inference()` で左から右への漸進的解決を行っている。
これには以下の制限がある：

1. **引数順序依存**: ラムダが先に来ると型パラメータが未解決のまま
2. **戻り値逆推論なし**: `val result: List<String> = (nums, f) map` で注釈が推論に使われない
3. **部分的 expected の伝搬不可**: 未解決パラメータがあると expected type を渡さない
4. **ハードコードされた map 返り値修正**: `if called_func_name == "map"` で特殊処理
5. **`infer_param_type_from_usage` の Int32 フォールバック**: CLAUDE.md の「No Silent Fallbacks」に違反

### 検討した2案

**案A: OSV型解決（Restrict固有）**
- 4フェーズ順次実行: Object → Verb → Args → Return
- OSV 語順を型推論のプロトコルとして利用
- 85-90% のケースで成立するが、コンストラクタ・引数なし関数・逆推論で破綻
- 破綻ケースごとに特殊処理が必要

**案B: 制約ベース双方向型チェック（採用）**
- 全引数から制約を収集し、ソルバーで一括解決
- 引数順序に非依存
- 戻り値逆推論が自然に動作
- OSV の利点（第一引数から解決が始まる）は制約ソルバーの挙動として自動的に発生

### 採用理由

1. OSV は制約ベースの特殊ケースになる（結果的に同じフロー）
2. 破綻ケース（None, コンストラクタ等）を特殊扱いしなくていい
3. Restrict のサブタイピング無し・アフィン型の特性で実装が単純（HM + associated types 程度）
4. OSV案のPhase別エラーの発想は `ConstraintOrigin` で再現可能

## 設計

### 新規データ構造

```rust
// src/type_constraints.rs (新規ファイル)

/// 推論中の型。未解決の型変数を含みうる
pub enum InferType {
    Known(TypedType),          // 確定した型
    Var(TypeVar),              // 未解決の型変数
    Projection {               // 関連型射影 (C::Mapped<U>)
        base: Box<InferType>,
        assoc_name: String,
        args: Vec<InferType>,
    },
}

/// 型制約
pub enum Constraint {
    /// 二つの型が等しい: T1 = T2
    TypeEquals {
        expected: InferType,
        actual: InferType,
        origin: ConstraintOrigin,
    },
    /// 型が form を採用している: C of Container
    HasForm {
        ty: InferType,
        form_name: String,
        origin: ConstraintOrigin,
    },
    /// 関連型射影の解決: List<T>::Mapped<U> = List<U>
    AssociatedTypeResolution {
        base_type: InferType,
        form_name: String,
        assoc_name: String,
        type_args: Vec<InferType>,
        result: InferType,
        origin: ConstraintOrigin,
    },
}

/// 制約の発生元（エラーメッセージ用）
pub struct ConstraintOrigin {
    pub span: Option<Span>,
    pub kind: ConstraintKind,
}

pub enum ConstraintKind {
    Argument { func_name: String, arg_index: usize },
    ReturnAnnotation { var_name: String },
    LambdaParam { param_name: String },
    LambdaReturn,
    FormBound { type_param: String },
    AssocTypeProjection { assoc_name: String },
    // ...
}
```

### 制約収集アルゴリズム（概要）

```
collect_function_call(name, args, expected_return):
    func_def = lookup_function(name)

    // 各型パラメータに型変数を割り当て
    type_vars = { tp.name: fresh_var() for tp in func_def.type_params }

    // form 制約を発行
    for tp in func_def.type_params:
        for form in tp.of_forms:
            emit HasForm(type_vars[tp.name], form)

    // 全引数の制約を収集（順序非依存）
    for (i, (arg, param_ty)) in zip(args, func_def.params):
        instantiated_param = substitute(param_ty, type_vars)
        arg_ty = collect_expr(arg, expected=Some(instantiated_param))
        emit TypeEquals(instantiated_param, arg_ty, Argument(name, i))

    // 戻り値型を構築
    return_ty = substitute(func_def.return_type, type_vars)
    final_return = resolve_projections(return_ty, type_vars)

    // 戻り値逆推論: 変数の型注釈があれば制約に追加
    if let Some(expected) = expected_return:
        emit TypeEquals(final_return, expected, ReturnAnnotation)

    return final_return
```

### 制約ソルバー（概要）

```
solve(constraints):
    substitution = {}
    worklist = constraints

    while changed:
        for constraint in worklist:
            apply current substitution to constraint
            match constraint:
                TypeEquals(t1, t2):
                    unify(t1, t2) → new bindings or defer
                HasForm(ty, form):
                    if ty is concrete: verify form_adoptions
                    else: defer
                AssociatedTypeResolution(base, form, assoc, args, result):
                    if base is concrete: resolve and bind result
                    else: defer

    // 残った未解決制約はエラー
    for deferred in worklist:
        emit error with ConstraintOrigin
```

### Restrict 固有の単純化

1. **サブタイピング無し**: unification は純粋な等値チェック。variance 不要
2. **アフィン型**: 各変数が最大1回使用 → 制約の競合が起こらない
3. **form のクローズドワールド**: form_adoptions は有限集合。完全なパターンマッチで解決
4. **コンストラクタ特殊処理不要**: `None` は expected type から、`Some(x)` は x から、どちらも制約として表現

### ラムダの双方向型チェック

```
collect_lambda(params, body, expected):
    match expected:
        Some(Known(Function { params: exp_params, return_type })):
            // 期待型から引数型が判明（完全 or 部分的）
            for (param, exp_ty) in zip(params, exp_params):
                bind_var(param, exp_ty)
            body_ty = collect_expr(body, expected=Some(return_type))

        Some(Var(v)):
            // 期待型が未解決変数 → 新しい変数を作って制約
            p_vars = [fresh_var() for _ in params]
            r_var = fresh_var()
            emit TypeEquals(Var(v), Function(p_vars, r_var))
            for (param, p_var) in zip(params, p_vars):
                bind_var(param, p_var)
            body_ty = collect_expr(body, expected=Some(r_var))

        None:
            // 文脈なし → エラー（No Silent Fallbacks）
            error("cannot infer lambda parameter types without context")
```

### 関連型射影

```
// map の戻り値型は C（TypeParam）だが、実際は C::Mapped<U>
// 制約ベースでは AssociatedTypeResolution として表現

resolve_return_projections(name, return_ty, type_vars):
    if needs_projection(name):  // map は Mapped<U> を使う
        result_var = fresh_var()
        emit AssociatedTypeResolution(
            base=type_vars["C"],
            form="Container",
            assoc="Mapped",
            args=[type_vars["U"]],
            result=result_var
        )
        return result_var
    else:
        return return_ty  // filter, forEach はそのまま
```

### エラーメッセージ

各制約は `ConstraintOrigin` を持ち、失敗時に文脈情報を提供：

```
// Phase 1相当（Object の型が不正）
error[E0031]: first argument is not a container type
  --> src/main.rl:5:2
   |
5  | (42, |x| x + 1) map
   | ^^ `Int` does not take form `Container`
   |
   = note: types that take `Container`: List, Option

// Phase 3相当（ラムダの型が不一致）
error[E0033]: argument 2 of 'map' expects |Int32| -> U, found |String| -> Int32
  --> src/main.rl:5:10
   |
   = note: element type `Int32` was determined from the first argument

// 逆推論のエラー
error[E0035]: cannot determine type parameter U
  --> src/main.rl:5:1
   |
5  | val result = (empty_list, f) map
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `U` is unresolved
   |
   = help: add a type annotation: `val result: List<String> = ...`
```

## 実装計画

### Phase 1: 制約インフラ追加

| ファイル | 変更 | LOC |
|---------|------|-----|
| `src/type_constraints.rs` | **新規**: InferType, Constraint, ConstraintSolver | +500 |
| `src/lib.rs` | `pub mod type_constraints;` 追加 | +1 |

### Phase 2: `check_function_call_with_inference` 置き換え

| ファイル | 変更 | LOC |
|---------|------|-----|
| `src/type_checker.rs` | 制約収集版に書き換え。`expected_return` 引数追加。ハードコード map fixup 削除 | +200 / -130 |

### Phase 3: ラムダの部分的 expected 対応

| ファイル | 変更 | LOC |
|---------|------|-----|
| `src/type_checker.rs` | `check_lambda_expr` を制約対応に修正。`infer_param_type_from_usage` 削除 | +80 / -120 |

### Phase 4: 関連型射影の汎化

| ファイル | 変更 | LOC |
|---------|------|-----|
| `src/type_checker.rs` | `AssociatedTypeConstructor` テーブル追加。`register_std_forms` で Mapped 登録 | +80 |

### テスト

| テスト | 内容 |
|--------|------|
| 戻り値逆推論 | `val r: List<String> = (nums, f) map` |
| 部分 expected | ラムダ引数型が解決済み・戻り値が未解決 |
| 関連型射影 | `map(List<Int>, |Int| -> String)` → `List<String>` |
| エラー品質 | 制約 origin に基づくメッセージ |
| forEach | `(nums, |x| (x) print_int) forEach` |
| Option | `(Some(42), |x| x + 1) map` → `Option<Int>` |

### 合計

| | 新規 | 変更 | 削除 | 差分 |
|---|---|---|---|---|
| LOC | +660 | +280 | -250 | **+690** |

## 他言語との比較

| 言語 | 方式 | Restrict との差 |
|------|------|----------------|
| Haskell | HM + 型クラス辞書 | Restrict はサブタイピング・HKT 不要で単純 |
| Rust | 制約 + trait solving + lifetime | Restrict は lifetime が別パス、サブタイピング無し |
| Kotlin | 制約 + 双方向 | 最も近い。ただし Kotlin は class hierarchy あり |
| Swift | 制約ソルバー | Restrict より複雑（coercion, overloading） |
| Scala 2 | 左→右 | 現在の Restrict と同等（弱い） |
| **Restrict (提案)** | **制約 + 双方向 + form** | **HM + associated types 程度の素朴さ** |

## Restrict 固有の利点

1. **サブタイピング無し** → solver は純粋 unification のみ
2. **アフィン型** → 変数が1箇所でしか使われないので制約の競合なし
3. **form のクローズドワールド** → 関連型解決がテーブルルックアップ
4. **OSV 語順** → 第一引数が具象型なので制約解決が高速に収束
5. **`f()` 記法の廃止** → 全呼び出しが `(args) verb` 形式なので構文解析が一様

## 関連 Issue

- Ischca/restrict_lang#32: 型システムの多相機構 form/takes/of の設計と導入
