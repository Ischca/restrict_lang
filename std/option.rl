// Standard Library: Option
//
// Pure-Restrict helpers for Option<T> complementing compiler-registered
// builtins (option_is_some, option_is_none, option_unwrap_or).

pub fun option_or: <T>(opt: Option<T>, fallback: Option<T>) -> Option<T> = {
    opt match {
        Some (value) => { (value) Option::Some }
        None => { fallback }
    }
}

pub fun option_unwrap_or_else: <T>(opt: Option<T>, fallback: () -> T) -> T = {
    opt match {
        Some (value) => { value }
        None => { () fallback }
    }
}

pub fun option_and_then: <T, U>(opt: Option<T>, f: T -> Option<U>) -> Option<U> = {
    opt match {
        Some (value) => { value |> f }
        None => { () Option::None }
    }
}

pub fun option_ok_or: <T, E>(opt: Option<T>, error: E) -> Result<T, E> = {
    opt match {
        Some (value) => { (value) Result::Ok }
        None => { (error) Result::Err }
    }
}

pub fun option_flatten: <T>(opt: Option<Option<T>>) -> Option<T> = {
    opt match {
        Some (inner) => { inner }
        None => { () Option::None }
    }
}

pub fun option_zip_with: <A, B, C>(left: Option<A>, right: Option<B>, combine: (A, B) -> C) -> Option<C> = {
    left match {
        Some (a) => {
            right match {
                Some (b) => { ((a, b) combine) Option::Some }
                None => { () Option::None }
            }
        }
        None => { () Option::None }
    }
}
