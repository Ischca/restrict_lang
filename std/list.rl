// Restrict Language Standard Library: List Operations
// 標準ライブラリ: リスト操作
//
// Note: Many list functions are not included due to parser/type checker limitations:
// - Parser doesn't support function type parameters (map, filter, fold)
// - Parser doesn't support nested generic return types
// - Recursive generic functions cause type checker issues

// ============================================================
// Basic Predicates
// ============================================================

// Check if list is empty
export fun is_empty: <T> (list: List<T>) -> Bool = {
    list match {
        [] => { true }
        _ => { false }
    }
}

// ============================================================
// Element Access
// ============================================================

// Get first element (None if empty)
export fun head: <T> (list: List<T>) -> Option<T> = {
    list match {
        [h | _] => { Some(h) }
        _ => { None }
    }
}

// ============================================================
// List Construction
// ============================================================

// Add element to front of list
export fun prepend: <T> (item: T, list: List<T>) -> List<T> = {
    [item | list]
}
