# Type Inference System Design: Constraint-Based Bidirectional Approach

## ステータス: 設計確定・段階0-1実装待ち

## 背景

### 現状の問題

現在の型推論は `check_function_call_with_inference()` で左から右への漸進的解決を行っている。
これには以下の制限がある：

1. **引数順序依存**: ラムダが先に来ると型パラメータが未解決のまま
2. **戻り値逆推論なし**: `val result: List<String> = (nums, f) map` で注釈が推論に使われない
3. **部分的 expected の伝搬不可**: 未解決パラメータがあると expected type を渡さない
4. **ハードコードされた map 返り値修正**: `if called_func_name == "map"` で特殊処理
5. **`infer_param_type_from_usage` の Int32 フォールバック**: CLAUDE.md の「No Silent Fallbacks」に違反

### 検討した案

| 案 | 概要 | 採否 |
|----|------|------|
| OSV型解決（固有） | 4フェーズ順次実行。Object→Verb→Args→Return | 不採用。85-90%成立するが破綻ケースで特殊処理が要る |
| 制約ベース双方向 | 全引数から制約収集、ソルバーで一括解決 | **採用** |

---

## 設計の核心：A層 / B層の分離

Restrict の型システム機能は2つの根本的に異なる層に分かれる。**混ぜると壊れる**。

### A層：Equational（等式的・単調・順序非依存）

- 型の等式制約 `T1 = T2`
- form 採用制約 `C of Container`
- associated type 射影 `C.Mapped<U>`
- 型変数の統一 (unification)
- 戻り値逆推論
- ラムダへの期待型伝搬

**性質**: substitution で解ける。一度成り立てば崩れない。**引数の順序は無関係**。

### B層：Substructural（部分構造的・非単調・評価順依存）

- アフィン消費 `R → R'`（各値は最大1回使用）
- context 可用性 `C`（`with` ブロック内で何が使えるか）
- temporal/lifetime `Θ`（`~f` の有効範囲）
- freeze/open/sealed（資源の封印）
- residual environment `⊣ Γ'`（消費後に残る環境）

**性質**: 環境スレッディング `Γ ⊢ e : T ⊣ Γ'`。`if` の両枝が同じ資源を消費せねばならない等、**等式制約では表現できない**。

### なぜ分離が重要か

アフィン消費を「制約バッグ」に入れると、型推論中の参照と実行意味上の消費が混ざる。
推論は同じ式を複数回走査しうるので、走査ごとに `used=true` が更新されるとアフィン違反が誤発生する。

**A層は制約を集めて解く。B層は評価順に環境を畳む。**

### OSV の役割（層による違い）

| | A層（型推論） | B層（資源フロー） |
|--|---|---|
| OSV の役割 | `Apply` 制約を生成する構文。推論順序ではない | 資源・文脈・環境の**自然な評価順**。Object が最初に消費される |
| 順序の意味 | 無関係（制約ソルバーが解く） | 重要（`⊣ Γ'` が左から右に流れる） |
| エラーメッセージ | 制約の `origin` で OSV 的文脈を復元 | 「ここで消費した値をここで再使用」と読み順で説明 |

> **OSVは「型推論の手順」ではなく「資源フロー検査の自然な評価順」。**
> A層は OSV に依存しない。B層は OSV と自然に整合する。

---

## A層の設計

### 型表現: `TypedType` に `InferVar` / `Projection` を追加

別 enum (`InferType`) ではなく、既存の `TypedType` に推論用バリアントを足す。
理由: `TypedType` は既に 15 バリアントを持ち、`format_typed_type`, `is_copy_type`, `TypeSubstitution::apply`, 
シンボルテーブル等に広く使われている。別 enum にすると全バリアントの二重定義 + 並行 unify のボイラープレートが重い。

```rust
pub enum TypedType {
    // ... 既存バリアント（Int32, Float64, Boolean, String, Char, Unit,
    //     Record, Function, Option, Result, List, Array, Tuple, Range,
    //     TypeParam, Temporal）...

    // 推論専用（A層内部でのみ使用。codegen に漏れてはならない）
    InferVar(TypeVarId),      // 未解決の型変数
    Projection {              // 関連型射影 (C.Mapped<U>)
        base: Box<TypedType>,
        form_name: String,
        assoc_name: String,
        args: Vec<TypedType>,
    },
}
```

### Finalize 境界（InferVar が codegen に漏れない保証）

```rust
pub fn finalize_type(ty: &TypedType, subst: &Substitution) -> Result<TypedType, TypeError> {
    let zonked = zonk(ty, subst)?;
    if contains_infer_var(&zonked) {
        return Err(TypeError::CannotInferType(format_typed_type(&zonked)));
    }
    if contains_projection(&zonked) {
        return Err(TypeError::UnresolvedProjection(format_typed_type(&zonked)));
    }
    Ok(zonked)
}
```

概念的な位相:
- `TypedType + InferVar` = A層内部型（推論中のみ有効）
- finalize 後の `TypedType`（InferVar/Projection を含まない）= codegen 入力型

### 制約表現

```rust
// src/type_constraints.rs

pub struct TypeVarId(u32);

pub enum Constraint {
    /// 二つの型が等しい: T1 = T2
    TypeEquals {
        expected: TypedType,
        actual: TypedType,
        origin: ConstraintOrigin,
    },
    /// 型が form を採用している: C of Container
    HasForm {
        ty: TypedType,
        form_name: String,
        origin: ConstraintOrigin,
    },
    /// 関連型射影の解決: List<T>::Mapped<U> = List<U>
    AssociatedTypeResolution {
        base_type: TypedType,
        form_name: String,
        assoc_name: String,
        type_args: Vec<TypedType>,
        result: TypedType,  // InferVar が入る
        origin: ConstraintOrigin,
    },
}

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
}
```

### 制約収集アルゴリズム

```
collect_function_call(name, args, expected_return):
    func_def = lookup_function(name)

    // 各型パラメータに型変数を割り当て
    type_vars = { tp.name: InferVar(fresh()) for tp in func_def.type_params }

    // form 制約を発行
    for tp in func_def.type_params:
        for form in tp.of_forms:
            emit HasForm(type_vars[tp.name], form)

    // 全引数の制約を収集（順序非依存）
    for (i, (arg, param_ty)) in zip(args, func_def.params):
        instantiated = substitute(param_ty, type_vars)
        arg_ty = collect_expr(arg, expected=Some(instantiated))
        emit TypeEquals(instantiated, arg_ty, Argument(name, i))

    // 戻り値型を構築（projection あり）
    return_ty = substitute(func_def.return_type, type_vars)
    final_return = build_return_with_projections(return_ty, type_vars)

    // 戻り値逆推論: 変数の型注釈があれば制約に追加
    if let Some(expected) = expected_return:
        emit TypeEquals(final_return, expected, ReturnAnnotation)

    return final_return
```

### ラムダの型チェック

```
collect_lambda(params, body, expected):
    expected = prune(expected)

    match expected:
        Some(Function { params: ps, return_type: r }):
            // 期待型から引数型が判明（完全 or 部分的）
            bind params to ps
            body_ty = collect_expr(body, expected = Some(r))
            emit TypeEquals(body_ty, r, LambdaReturn)
            return Function(ps, r)

        Some(InferVar(v)):
            // 期待型が未解決変数 → 新しい変数を作って制約
            ps = [InferVar(fresh()) for _ in params]
            r = InferVar(fresh())
            emit TypeEquals(InferVar(v), Function(ps, r))
            bind params to ps
            body_ty = collect_expr(body, expected = Some(r))
            emit TypeEquals(body_ty, r, LambdaReturn)
            return Function(ps, r)

        None:
            // v1: 文脈なしラムダはエラー（No Silent Fallbacks）
            error("cannot infer lambda parameter types without context")
```

### 制約ソルバー

```
solve(constraints):
    substitution = {}
    worklist = constraints
    changed = true

    while changed:
        changed = false
        deferred = []

        for c in worklist:
            c = apply_substitution(c)
            match c:
                TypeEquals(t1, t2):
                    match unify(t1, t2):
                        Ok(bindings) → add to substitution; changed = true
                        Err if both ground → error
                        Err → deferred.push(c)

                HasForm(ty, form):
                    match ty:
                        concrete → verify via goal-directed solving (see below)
                        InferVar → deferred.push(c)

                AssociatedTypeResolution(base, form, assoc, args, result):
                    match base:
                        concrete → resolve from adoption table; changed = true
                        InferVar → deferred.push(c)

        worklist = deferred

    // 残った未解決制約はエラー
    for c in worklist:
        emit error with ConstraintOrigin
```

### Form Solving: closed-world goal-directed resolution

単純な `table[type_name][form_name]` lookup ではない。条件付き adoption があるため。

```
// 直接 adoption
List<T> takes Container<T>      →  テーブルに直接登録

// 条件付き adoption
List<T> takes Printable where T of Printable
→ Horn ルール: List<T> of Printable :- T of Printable

solve_form_goal(ty, form):
    1. 直接 adoption があるか → OK
    2. 条件付き adoption があるか
    3. where 条件を subgoal として再帰解決
    4. closed-world: 候補がなければ失敗

制限:
    - closed-world（宣言されたものだけ）
    - overlapping adoption は当面禁止
    - 再帰深度制限
    - negative constraints なし
    - where は form goal のみ
```

### Associated Type Projection: シグネチャ駆動

`map` の返り値を `if called_func_name == "map"` で修正するのではなく、
**form のシグネチャに associated type を埋め込む**。

```restrict
form Container<Self> {
    type Item           // 要素型
    type Mapped<U>      // 要素型を U に変えた同じコンテナ

    map: (Self, Item -> U) -> Mapped<U>
    filter: (Self, Item -> Bool) -> Self
    forEach: (Self, Item -> Unit) -> Unit
}

List<T> takes Container {
    type Item = T
    type Mapped<U> = List<U>
}
```

制約として：
```
HasForm(C, Container)
C::Item = T           // adoption table から解決
f : T -> U
result : C::Mapped<U>  // adoption table + U から解決
```

`List<Int>` なら `Mapped<String> = List<String>`。
`Option<Int>` なら `Mapped<String> = Option<String>`。
関数名に依存しない。将来の `flatMap`, `collect`, `zip` も同じ仕組み。

---

## B層の設計（北極星）

B層は v1 では大きく変更しない。既存のアフィンチェック・context チェックを維持する。
将来の方向を北極星として記録する。

### 型判断の形式

```
Γ ; C ; R ; Θ ⊢ expr : T ⊣ Γ' ; C' ; R' ; Θ'

Γ  = 変数環境
C  = context/capability 環境
R  = resource/affine usage 環境
Θ  = temporal/lifetime 環境

expr は T を返し、評価後の環境は Γ', C', R', Θ' になる
```

### `freeze` — 4環境を同時に閉じる代表操作

```
freeze : Open<resource R, requires C, in Θ> → Sealed<hash>
```

- R を消費する（open resource が sealed value へ）
- C を確定する（context 依存が漏れるか封じられるかを型で表現）
- Θ を閉じる（temporal scope を終了）
- hash identity を確定する

これが Restrict の型推論を他言語と差別化する**ショーケース**。

### v1 の B層方針

- アフィンチェック: **推論から分離**。型推論中は `used` を更新しない。推論後の usage graph で検査
- context: 既存の `with` / `@Context` 仕組みを維持。効果多相は v1 では入れない
- temporal: 既存の `lifetime_inference.rs` を維持。TAT は v2.0

### 将来の B層拡張（段階5以降）

- **効果推論**: `A -{Context}-> B` 関数型。効果変数と効果多相（Koka的）
- **linearity-guided generalization**: `val id = |x| x` を `∀a. a→a` にする。一般化できるのは copy/pure/no-affine-capture/no-context-leak の束縛だけ。B層が「いつ一般化が健全か」を判定
- **residual record**: `bundle.token take` でフィールド単位の残余型。ただし現在の nominal + hash record と衝突するため、別枠（resource bundle / structural capability object）として検討
- **freeze の4環境型**: `Open<R, C, Θ> → Sealed<hash>` の完全な型表現

---

## Restrict 固有の単純化

| 特性 | 効果 |
|------|------|
| サブタイピング無し | solver は純粋 unification のみ。variance 不要 |
| アフィン型 | 変数が1箇所でしか使われないので A層の制約競合なし。B層で消費を検査 |
| form のクローズドワールド | associated type 解決がゴール指向テーブルルックアップ |
| OSV 語順 | 第一引数が具象型→制約解決が高速収束。B層では自然な評価順 |
| `f()` 記法の廃止 | 全呼び出しが `(args) verb` 形式。構文解析が一様 |

---

## 実装計画（段階的）

### 段階0: InferVar + Constraint インフラ（A層のみ）

**方針**: 既存の型検査を壊さず、新しいモジュールを追加するだけ。

| ファイル | 変更 |
|---------|------|
| `src/type_constraints.rs` | **新規**: TypeVarId, Constraint, ConstraintOrigin, Substitution, fresh_var, unify, zonk, finalize_type |
| `src/ast.rs` | `TypedType` に `InferVar(TypeVarId)`, `Projection{...}` を追加 |
| `src/lib.rs` | `pub mod type_constraints;` |

この時点では `map` 等は触らない。

### 段階1: ラムダを新推論に乗せる

| ファイル | 変更 |
|---------|------|
| `src/type_checker.rs` | `check_lambda_expr` を制約対応に。`infer_param_type_from_usage` 削除。文脈なしラムダはエラー |

到達点:
```restrict
val inc: Int -> Int = |x| x + 1   // OK: expected type から x: Int
val id = |x| x                     // Error: cannot infer without context
```

### 段階2: call/OSV/pipe を Apply に正規化

構文ごとの差を推論から切り離す。`(a, b) f` / `a |> f(b)` が内部的には同じ `Apply(f, [a, b])` になる。

### 段階3: form / associated type projection をシグネチャ駆動に

| ファイル | 変更 |
|---------|------|
| `src/type_checker.rs` | `register_std_forms` で `Item`, `Mapped<U>` を form に登録。`map` の hardcoded fixup 削除 |
| `src/type_constraints.rs` | `HasForm` の goal-directed solving 実装 |

到達点:
```restrict
val r: List<String> = (nums, |n| (n) int_to_string) map
// 戻り値逆推論で U=String が確定。返り値は List<String>
```

### 段階4: アフィンを `⊣ Γ'` 残余環境に移す（B層最初の改善）

| ファイル | 変更 |
|---------|------|
| `src/type_checker.rs` | 推論中の `used` 更新を停止。usage graph を収集し、推論後に検査 |

### 段階5以降（別Issue）

- context/`with` を効果として推論（効果変数・多相）
- temporal Θ 統合
- freeze の4環境型
- linearity-guided generalization
- residual record

### 合計見積もり（段階0-3）

| | 新規 | 変更 | 削除 | 差分 |
|---|---|---|---|---|
| LOC | +600 | +300 | -250 | **+650** |

---

## 他言語との比較

| 言語 | 方式 | Restrict との差 |
|------|------|----------------|
| Haskell | HM + 型クラス辞書 | サブタイピング・HKT 不要で Restrict は単純 |
| Rust | 制約 + trait solving + lifetime | lifetime が別パス、サブタイピング無しで Restrict は単純 |
| Kotlin | 制約 + 双方向 | 最も近い。ただし class hierarchy あり |
| Swift | 制約ソルバー | coercion, overloading で Restrict より複雑 |
| Scala 2 | 左→右 | 現在の Restrict と同等（弱い） |
| **Restrict (v1)** | **制約 + 双方向 + form** | **HM + associated types 程度の素朴さ。B層は分離** |

---

## v1 の制限（意図的）

| 制限 | 理由 |
|------|------|
| 文脈なしラムダはエラー | 一般化にはB層（アフィン安全性判定）が必要 |
| 効果推論なし | 効果多相は段階5以降 |
| residual record なし | nominal+hash record と衝突。別枠で検討 |
| `val id = \|x\| x` は通らない | linearity-guided generalization は北極星 |

---

## 関連 Issue

- Ischca/restrict_lang#32: 型システムの多相機構 form/takes/of の設計と導入
