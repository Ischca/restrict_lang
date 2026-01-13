// Restrict Language Standard Library: List Operations
// 標準ライブラリ: リスト操作

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

// Get list without first element (None if empty)
export fun tail: <T> (list: List<T>) -> Option<List<T>> = {
    list match {
        [_ | t] => { Some(t) }
        _ => { None }
    }
}

// ============================================================
// Length
// ============================================================

// Get length of list
export fun length: <T> (list: List<T>) -> Int = {
    (list, 0) length_helper
}

// Helper for length calculation (tail recursive)
fun length_helper: <T> (list: List<T>, acc: Int) -> Int = {
    list match {
        [_ | t] => { (t, acc + 1) length_helper }
        _ => { acc }
    }
}

// ============================================================
// List Construction
// ============================================================

// Add element to front of list
export fun prepend: <T> (item: T, list: List<T>) -> List<T> = {
    [item | list]
}

// ============================================================
// List Operations
// ============================================================

// Reverse a list
export fun reverse: <T> (list: List<T>) -> List<T> = {
    (list, []) reverse_helper
}

// Helper for reverse (tail recursive)
fun reverse_helper: <T> (list: List<T>, acc: List<T>) -> List<T> = {
    list match {
        [h | t] => { (t, [h | acc]) reverse_helper }
        _ => { acc }
    }
}

// Concatenate two lists
export fun concat: <T> (a: List<T>, b: List<T>) -> List<T> = {
    a match {
        [h | t] => { [h | (t, b) concat] }
        _ => { b }
    }
}

// ============================================================
// Higher-Order Functions
// ============================================================

// Apply function to each element
export fun map: <T, U> (list: List<T>, f: |T| -> U) -> List<U> = {
    list match {
        [h | t] => { [(h) f | (t, f) map] }
        _ => { [] }
    }
}

// Filter list by predicate
export fun filter: <T> (list: List<T>, pred: |T| -> Bool) -> List<T> = {
    list match {
        [h | t] => {
            (h) pred then {
                [h | (t, pred) filter]
            } else {
                (t, pred) filter
            }
        }
        _ => { [] }
    }
}

// Fold list from left
export fun fold: <T, U> (list: List<T>, acc: U, f: |U, T| -> U) -> U = {
    list match {
        [h | t] => { (t, (acc, h) f, f) fold }
        _ => { acc }
    }
}

// ============================================================
// Option-returning Operations
// ============================================================

// Flatten nested Option
export fun flatten: <T> (opt: Option<Option<T>>) -> Option<T> = {
    opt match {
        Some(inner) => { inner }
        _ => { None }
    }
}
