// =============================================================================
// form_container.rl -- Container form and generic collection operations
// =============================================================================
//
// This file demonstrates the "form" system in Restrict Language.
// NOTE: This is a design document / example. Code generation for forms is
//       still being implemented, so this file is not yet compilable.
//
// ---------------------------------------------------------------------------
// What is a form?
// フォーム(form)とは何か?
// ---------------------------------------------------------------------------
//
// A form describes a set of capabilities that a type can provide. It is
// similar to Rust traits or Haskell type-classes, but with key differences:
//
//   1. Forms support associated types, allowing a form adoption to specify
//      concrete types that depend on the adopter.
//      (フォームは関連型をサポートし、採用者に依存する具体的な型を指定できる。)
//
//   2. The "takes" keyword is used instead of "impl" -- a type *takes on*
//      the shape of a form. This language emphasises that the type is
//      choosing to conform, not inheriting behavior.
//      ("takes" キーワードは型がフォームの形を *引き受ける* ことを強調する。)
//
//   3. Forms are monomorphised at compile time. When a function declares
//      `x of Container<T>`, the compiler generates a specialised version
//      for each concrete type that appears at call sites. There is no
//      vtable or dynamic dispatch -- every call resolves statically.
//      (フォームはコンパイル時に単相化される。動的ディスパッチは存在しない。)
//
//   4. Form algebra (Container<T> + Printable) allows combining constraints.
//      The compiler intersects the required methods and verifies that the
//      concrete type satisfies all of them.
//      (フォーム代数により制約の組み合わせが可能。)
//
// ---------------------------------------------------------------------------
// The "takes" mechanism / "takes" メカニズム
// ---------------------------------------------------------------------------
//
// When we write `List<T> takes Container<T>`, we are declaring:
//   - List<T> provides every method that Container<T> requires
//   - List<T> supplies concrete definitions for every associated type
//   - The compiler checks these at declaration time, not at use time
//
// This is an *explicit opt-in*. A type never satisfies a form implicitly.
// (型は暗黙的にフォームを満たすことはない。明示的なオプトインが必要。)
//
// ---------------------------------------------------------------------------
// The "of" constraint / "of" 制約
// ---------------------------------------------------------------------------
//
// In function signatures, `x of Container<T>` means:
//   "x can be any type that has taken Container<T>"
//
// The compiler uses this to:
//   1. Verify that every form method used in the body is available
//   2. Resolve associated types (e.g., container.Mapped<U>)
//   3. Generate monomorphised code for each concrete type at call sites
//
// (コンパイラはこの制約を使って、メソッドの利用可能性を検証し、
//  関連型を解決し、呼び出し箇所ごとに単相化コードを生成する。)
//
// ---------------------------------------------------------------------------
// Monomorphisation / 単相化
// ---------------------------------------------------------------------------
//
// At compile time, a function like:
//     fun map(container of Container<T>, f: |T| -> U) -> container.Mapped<U>
//
// is expanded for each call site. If we call:
//     (myList, |x| x + 1) map
//     (myOption, |x| x * 2) map
//
// The compiler emits two specialised functions:
//     fun map_List_Int32(container: List<Int32>, f: |Int32| -> Int32) -> List<Int32>
//     fun map_Option_Int32(container: Option<Int32>, f: |Int32| -> Int32) -> Option<Int32>
//
// No runtime indirection. This is critical for WASM without GC.
// (ランタイムの間接参照はない。GCなしのWASMにとって重要。)
//
// ---------------------------------------------------------------------------
// Future directions / 今後の方向性
// ---------------------------------------------------------------------------
//
//   - Form algebra: combining forms with + (intersection of capabilities)
//     (フォーム代数: + による能力の交差)
//
//   - Scope-based visibility: forms can be adopted in a limited scope,
//     preventing "orphan instance" problems. An adoption is only visible
//     within the module that declares it.
//     (スコープベースの可視性: フォームの採用を限定的なスコープに制限可能)
//
//   - Default method bodies: forms may provide default implementations
//     that adopters can override.
//     (デフォルトメソッド本体: フォームがデフォルト実装を提供可能)
//
//   - Negative constraints: `x of !Cloneable` to assert a type does NOT
//     satisfy a form, useful for enforcing move-only semantics.
//     (否定制約: 型がフォームを満たさないことの表明)
//
// =============================================================================


// =============================================================================
// 1. Container form definition
//    コンテナフォームの定義
// =============================================================================
//
// Container<T> captures the minimal interface for a foldable, buildable
// collection. Everything else (map, filter, forEach) can be derived from
// fold + empty + append.
//
// - fold<U>   : collapse all elements into a single value of type U
// - empty<U>  : produce an empty container (with potentially different elem type)
// - append    : add an element, returning a new container (affine-safe)

form Container<T> {
    // Associated type: what does this container look like when its elements
    // change from T to U?
    // 関連型: 要素がTからUに変わったとき、このコンテナはどうなるか?
    type Mapped<U>

    // Fold (reduce/catamorphism). The fundamental traversal operation.
    // 畳み込み(カタモーフィズム)。基本的な走査操作。
    fold<U>: (self, init: U, f: |U, T| -> U) -> U

    // Construct an empty container whose elements would be of type U.
    // 要素型Uの空コンテナを構築する。
    empty<U>: () -> Mapped<U>

    // Append a single element, producing a new container (no mutation).
    // 単一要素を追加し、新しいコンテナを返す(ミューテーションなし)。
    append: (self, elem: T) -> Self
}


// =============================================================================
// 2. List<T> takes Container<T>
//    リストがコンテナフォームを採用する
// =============================================================================
//
// List is the classic linked list. Pattern matching on [head | tail] is
// used for recursive traversal.

List<T> takes Container<T> {
    // When we map over a List<T> with f: |T| -> U, we get a List<U>.
    // List<T>にf: |T| -> Uを適用すると、List<U>が得られる。
    type Mapped<U> = List<U>

    // Fold by recursive pattern match on the list structure.
    // The OSV syntax reads as:
    //   (t, (init, h) f, f) fold  -->  t.fold(f(init, h), f)
    //
    // リスト構造の再帰的パターンマッチによる畳み込み。
    fold = |self, init, f| {
        self match {
            // Non-empty list: fold the head into the accumulator, recurse on tail
            // 非空リスト: headをアキュムレータに畳み込み、tailで再帰
            [h | t] => (t, (init, h) f, f) fold

            // Empty list: return the accumulated value
            // 空リスト: 蓄積された値を返す
            [] => init
        }
    }

    // An empty List<U>.
    // 空のList<U>。
    empty = || { [] }

    // Append by consing to front and reversing.
    // In a real implementation this could use a more efficient structure.
    // 先頭にconsして反転することで追加する。
    append = |self, elem| { [elem | self] reverse }
}


// =============================================================================
// 3. Option<T> takes Container<T>
//    OptionがContainerフォームを採用する
// =============================================================================
//
// Option is a container of 0 or 1 elements. This adoption lets us reuse
// the same generic map/filter/forEach for optional values.

Option<T> takes Container<T> {
    // Mapping over Option<T> with f: |T| -> U gives Option<U>.
    // Option<T>にf: |T| -> Uを適用するとOption<U>が得られる。
    type Mapped<U> = Option<U>

    // Fold: apply f if Some, otherwise return init.
    // 畳み込み: Someならfを適用、そうでなければinitを返す。
    fold = |self, init, f| {
        self match {
            Some(v) => (init, v) f
            None => init
        }
    }

    // Empty Option is always None.
    // 空のOptionは常にNone。
    empty = || { None }

    // Appending to an Option replaces its content.
    // Optionへの追加はその内容を置き換える。
    append = |self, elem| { Some(elem) }
}


// =============================================================================
// 4. Generic higher-order functions using "of Container"
//    "of Container" を使ったジェネリック高階関数
// =============================================================================
//
// These functions work with ANY type that takes Container<T>.
// The `of` keyword constrains the parameter to types satisfying the form.
//
// これらの関数はContainer<T>を採用した任意の型で動作する。
// `of` キーワードはパラメータをフォームを満たす型に制約する。

// map: transform every element with f, producing a new container.
// map: 各要素をfで変換し、新しいコンテナを生成する。
//
// Note the return type `container.Mapped<U>` -- this is an associated type
// projection. For List<T> it resolves to List<U>; for Option<T>, Option<U>.
// 戻り値型 `container.Mapped<U>` は関連型射影である。
fun map(container of Container<T>, f: |T| -> U) -> container.Mapped<U> = {
    (container, container.empty<U>(), |acc, elem| {
        (acc, (elem) f) append
    }) fold
}

// filter: keep only elements satisfying the predicate.
// filter: 述語を満たす要素のみを残す。
//
// The return type is the same container type (not Mapped), since element
// type does not change.
// 要素型は変わらないため、戻り値は同じコンテナ型。
fun filter(container of Container<T>, pred: |T| -> Bool) -> container = {
    (container, container.empty<T>(), |acc, elem| {
        (elem) pred then { (acc, elem) append } else { acc }
    }) fold
}

// forEach: execute a side-effecting function on every element.
// forEach: 各要素に副作用のある関数を実行する。
fun forEach(container of Container<T>, f: |T| -> Unit) -> Unit = {
    (container, (), |_, elem| { (elem) f }) fold
}

// sum: add all elements in a numeric container.
// sum: 数値コンテナの全要素を合計する。
fun sum(container of Container<Int32>) -> Int32 = {
    (container, 0, |acc, x| { acc + x }) fold
}

// count: how many elements are in the container.
// count: コンテナ内の要素数を返す。
fun count(container of Container<T>) -> Int32 = {
    (container, 0, |acc, _| { acc + 1 }) fold
}


// =============================================================================
// 5. Usage examples with OSV syntax
//    OSV構文による使用例
// =============================================================================
//
// Remember: in Restrict Language, the word order is Object-Subject-Verb.
//   (args) function_name   means   function_name(args)
//   obj method              means   method(obj)

fun main() = {
    // --- List operations ---

    val numbers = [1, 2, 3, 4, 5];

    // Double every element:  map(numbers, |x| x * 2)
    // 全要素を2倍にする
    val doubled = (numbers, |x| x * 2) map;

    // Keep only even numbers:  filter(numbers, |x| x % 2 == 0)
    // 偶数のみを残す
    val evens = (numbers, |x| x % 2 == 0) filter;

    // Print each doubled value
    // 2倍にした各値を表示する
    (doubled, |x| (x) println) forEach;

    // Sum the original list
    // 元のリストを合計する
    val total = (numbers) sum;

    // Count elements
    // 要素数を数える
    val n = (numbers) count;


    // --- Option operations ---

    val maybe = Some(42);

    // Map over an Option: Some(42) becomes Some(43)
    // Optionに対するmap: Some(42)がSome(43)になる
    val incremented = (maybe, |x| x + 1) map;

    // Filter on Option: keeps Some(42) because 42 is even
    // Optionに対するfilter: 42は偶数なのでSome(42)を保持
    val still_some = (maybe, |x| x % 2 == 0) filter;

    // Filter that eliminates: None, because 42 is not > 100
    // 除外するfilter: 42は100より大きくないのでNone
    val gone = (maybe, |x| x > 100) filter;

    // forEach on Option: prints 42 exactly once
    // Optionに対するforEach: 42をちょうど1回表示する
    (maybe, |x| (x) println) forEach;


    // --- Chaining with pipe operator ---
    // パイプ演算子によるチェーン

    // |> creates an immutable binding for the intermediate result
    // |> は中間結果の不変バインディングを作成する
    numbers
        |> (_, |x| x * 3) map
        |> (_, |x| x > 5) filter
        |> (_, |x| (x) println) forEach
}


// =============================================================================
// 6. Additional forms -- extensibility examples
//    追加フォーム -- 拡張性の例
// =============================================================================

// Printable: any type that can be converted to a string representation.
// Printable: 文字列表現に変換できる任意の型。
form Printable {
    // Convert the value to its display string.
    // 値をその表示文字列に変換する。
    to_string: (self) -> String
}

// Int32 takes Printable
Int32 takes Printable {
    to_string = |self| { (self) int_to_string }
}

// List<T> takes Printable when T itself is Printable.
// This is a conditional adoption: it only applies when the constraint holds.
// T自体がPrintableの場合にのみList<T>がPrintableを採用する。
// これは条件付き採用であり、制約が成り立つ場合にのみ適用される。
List<T> takes Printable where T of Printable {
    to_string = |self| {
        val inner = (self, "", |acc, elem| {
            acc == "" then { (elem) to_string }
                      else { acc ++ ", " ++ (elem) to_string }
        }) fold;
        "[" ++ inner ++ "]"
    }
}

// Comparable: types that support ordering.
// Comparable: 順序付けをサポートする型。
form Comparable {
    // Returns -1 if self < other, 0 if equal, 1 if self > other.
    // self < other なら -1、等しければ 0、self > other なら 1 を返す。
    compare: (self, other: Self) -> Int32
}

Int32 takes Comparable {
    compare = |self, other| {
        self < other then { -1 }
        else {
            self == other then { 0 } else { 1 }
        }
    }
}

// Sortable: a container whose elements can be compared.
// Sortable: 要素が比較可能なコンテナ。
form Sortable<T> {
    sort: (self) -> Self
}

// Generic min function using Comparable.
// Comparableを使ったジェネリックmin関数。
fun min(a of Comparable, b of Comparable) -> a = {
    (a, b) compare < 0 then { a } else { b }
}

// Generic max function.
// ジェネリックmax関数。
fun max(a of Comparable, b of Comparable) -> a = {
    (a, b) compare > 0 then { a } else { b }
}


// =============================================================================
// 7. Form composition (form algebra)
//    フォーム合成(フォーム代数)
// =============================================================================
//
// The `+` operator on form constraints creates an intersection requirement.
// The type must satisfy ALL listed forms.
//
// `+` 演算子はフォーム制約の交差要件を作成する。
// 型はリストされたすべてのフォームを満たす必要がある。

// process: works with any container whose elements are printable.
// process: 要素がprintableな任意のコンテナで動作する。
fun process(data of Container<T> + Printable) = {
    // We can use Container methods (fold, empty, append)
    // AND Printable methods (to_string) on data.
    // dataに対してContainerメソッド(fold, empty, append)と
    // Printableメソッド(to_string)の両方を使用できる。

    val description = (data) to_string;
    (description) println;

    val n = (data) count;
    (n) println
}

// display_sorted: requires Container + Printable for the container,
// and Comparable for the elements.
// display_sorted: コンテナにはContainer + Printable、要素にはComparableを要求する。
fun display_sorted(data of Container<T> + Printable, _unused: T of Comparable) = {
    // Sort (if the container is also Sortable), then print
    // ソート(コンテナがSortableでもある場合)して表示
    val sorted_data = (data) sort;
    val text = (sorted_data) to_string;
    (text) println
}


// =============================================================================
// Summary / まとめ
// =============================================================================
//
// Forms provide:
//   - Explicit capability declaration (form X { ... })
//   - Explicit adoption by types (T takes X { ... })
//   - Compile-time constraint checking (x of X)
//   - Associated type projection (x.Mapped<U>)
//   - Form algebra for combining constraints (X + Y)
//   - Monomorphisation for zero-cost abstraction
//
// フォームが提供するもの:
//   - 明示的な能力宣言 (form X { ... })
//   - 型による明示的な採用 (T takes X { ... })
//   - コンパイル時の制約チェック (x of X)
//   - 関連型射影 (x.Mapped<U>)
//   - 制約を組み合わせるためのフォーム代数 (X + Y)
//   - ゼロコスト抽象化のための単相化
//
// Because Restrict Language compiles to WASM without GC, monomorphisation
// is essential: every generic function becomes a set of concrete functions
// with fully resolved types. No boxing, no vtables, no runtime overhead.
//
// Restrict LanguageはGCなしのWASMにコンパイルされるため、単相化は不可欠である。
// すべてのジェネリック関数は、型が完全に解決された具象関数の集合になる。
// ボクシングなし、vtableなし、ランタイムオーバーヘッドなし。
