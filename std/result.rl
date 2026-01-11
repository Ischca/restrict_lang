// Restrict Language Standard Library: Result Operations
// 標準ライブラリ: Result操作

// ============================================================
// Basic Predicates
// ============================================================

// Check if result is Ok
export fun is_ok: <T, E> (result: Result<T, E>) -> Bool = {
    result match {
        Ok(_) => { true }
        Err(_) => { false }
    }
}

// Check if result is Err
export fun is_err: <T, E> (result: Result<T, E>) -> Bool = {
    result match {
        Ok(_) => { false }
        Err(_) => { true }
    }
}

// ============================================================
// Value Extraction
// ============================================================

// Get the Ok value or a default
export fun unwrap_or: <T, E> (result: Result<T, E>, default: T) -> T = {
    result match {
        Ok(value) => { value }
        Err(_) => { default }
    }
}

// Get the Err value or a default
export fun unwrap_err_or: <T, E> (result: Result<T, E>, default: E) -> E = {
    result match {
        Ok(_) => { default }
        Err(e) => { e }
    }
}

// ============================================================
// Transformations
// ============================================================

// Map the Ok value
export fun map_ok: <T, U, E> (result: Result<T, E>, f: |T| -> U) -> Result<U, E> = {
    result match {
        Ok(value) => { Ok((value) f) }
        Err(e) => { Err(e) }
    }
}

// Map the Err value
export fun map_err: <T, E, F> (result: Result<T, E>, f: |E| -> F) -> Result<T, F> = {
    result match {
        Ok(value) => { Ok(value) }
        Err(e) => { Err((e) f) }
    }
}

// Flat map (and_then) - chain Result-returning functions
export fun and_then: <T, U, E> (result: Result<T, E>, f: |T| -> Result<U, E>) -> Result<U, E> = {
    result match {
        Ok(value) => { (value) f }
        Err(e) => { Err(e) }
    }
}

// Convert Result to Option (discarding error)
export fun ok: <T, E> (result: Result<T, E>) -> Option<T> = {
    result match {
        Ok(value) => { Some(value) }
        Err(_) => { None }
    }
}

// Convert Result to Option (keeping error)
export fun err: <T, E> (result: Result<T, E>) -> Option<E> = {
    result match {
        Ok(_) => { None }
        Err(e) => { Some(e) }
    }
}
