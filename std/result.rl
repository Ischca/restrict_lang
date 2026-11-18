// Standard Library: Result
//
// Pure-Restrict helpers for Result<T, E>.

pub fun result_is_ok: <T, E>(res: Result<T, E>) -> Boolean = {
    res match {
        Ok(value) => { true }
        Err(err) => { false }
    }
}

pub fun result_is_err: <T, E>(res: Result<T, E>) -> Boolean = {
    res match {
        Ok(value) => { false }
        Err(err) => { true }
    }
}

pub fun result_unwrap_or: <T, E>(res: Result<T, E>, fallback: T) -> T = {
    res match {
        Ok(value) => { value }
        Err(err) => { fallback }
    }
}

pub fun result_unwrap_or_else: <T, E>(res: Result<T, E>, recover: E -> T) -> T = {
    res match {
        Ok(value) => { value }
        Err(err) => { err |> recover }
    }
}

pub fun result_map: <T, U, E>(res: Result<T, E>, f: T -> U) -> Result<U, E> = {
    res match {
        Ok(value) => { (value |> f) Result::Ok }
        Err(err) => { (err) Result::Err }
    }
}

pub fun result_map_err: <T, E, F>(res: Result<T, E>, f: E -> F) -> Result<T, F> = {
    res match {
        Ok(value) => { (value) Result::Ok }
        Err(err) => { (err |> f) Result::Err }
    }
}

pub fun result_and_then: <T, U, E>(res: Result<T, E>, f: T -> Result<U, E>) -> Result<U, E> = {
    res match {
        Ok(value) => { value |> f }
        Err(err) => { (err) Result::Err }
    }
}

pub fun result_ok: <T, E>(res: Result<T, E>) -> Option<T> = {
    res match {
        Ok(value) => { (value) Option::Some }
        Err(err) => { () Option::None }
    }
}

pub fun result_err: <T, E>(res: Result<T, E>) -> Option<E> = {
    res match {
        Ok(value) => { () Option::None }
        Err(err) => { (err) Option::Some }
    }
}
