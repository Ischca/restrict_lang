// Restrict Language Standard Library: Option Operations
// 標準ライブラリ: Option型操作
//
// Note: Some functions (map, flatten, or_else) are not included because
// the parser doesn't yet support nested generic types (Option<Option<T>>)
// or function type parameters (|T| -> U).

// ============================================================
// Basic Predicates
// ============================================================

// Check if Option contains a value
export fun is_some: <T> (opt: Option<T>) -> Bool = {
    opt match {
        Some(_) => { true }
        None => { false }
    }
}

// Check if Option is empty
export fun is_none: <T> (opt: Option<T>) -> Bool = {
    opt match {
        Some(_) => { false }
        None => { true }
    }
}

// ============================================================
// Unwrapping
// ============================================================

// Get value or return default
export fun unwrap_or: <T> (opt: Option<T>, default: T) -> T = {
    opt match {
        Some(value) => { value }
        None => { default }
    }
}
