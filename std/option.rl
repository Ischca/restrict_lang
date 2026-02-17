// Restrict Language Standard Library: Option Operations
// 標準ライブラリ: Option型操作

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

// ============================================================
// Transformations
// ============================================================

// Map a function over the Option value
export fun option_map: <T, U> (opt: Option<T>, f: |T| -> U) -> Option<U> = {
    opt match {
        Some(value) => { Some((value) f) }
        None => { None }
    }
}

// Flat map (and_then) - chain Option-returning functions
export fun option_and_then: <T, U> (opt: Option<T>, f: |T| -> Option<U>) -> Option<U> = {
    opt match {
        Some(value) => { (value) f }
        None => { None }
    }
}

// Return the option if Some, otherwise evaluate fallback
export fun option_or_else: <T> (opt: Option<T>, fallback: || -> Option<T>) -> Option<T> = {
    opt match {
        Some(value) => { Some(value) }
        None => { fallback }
    }
}
