// A closed enum gives Result a domain-specific error type.
enum DecodeError {
    Invalid(String)
}

fun decode: (code: Int32) -> Result<Int32, DecodeError> = {
    code == 0 then {
        Ok(42)
    } else {
        Err("invalid code" |> DecodeError::Invalid)
    }
}

fun explain: (result: Result<Int32, DecodeError>) -> String = {
    result match {
        Ok(value) => { "valid code" }
        Err(error) => {
            error match {
                DecodeError::Invalid(message) => { message }
            }
        }
    }
}

fun main: () = {
    1 |> decode |> explain |> println
}
