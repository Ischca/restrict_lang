// Standard Library: Char
//
// Pure-Restrict ASCII character classification and conversion helpers built
// on compiler-registered `char_code` and `char_from_code` builtins.

pub fun is_ascii_digit: (c: Char) -> Boolean = {
    val code = c |> char_code;
    (code >= 48) && (code <= 57)
}

pub fun is_ascii_upper: (c: Char) -> Boolean = {
    val code = c |> char_code;
    (code >= 65) && (code <= 90)
}

pub fun is_ascii_lower: (c: Char) -> Boolean = {
    val code = c |> char_code;
    (code >= 97) && (code <= 122)
}

pub fun is_ascii_alpha: (c: Char) -> Boolean = {
    (c |> is_ascii_upper) || (c |> is_ascii_lower)
}

pub fun is_ascii_alphanumeric: (c: Char) -> Boolean = {
    (c |> is_ascii_alpha) || (c |> is_ascii_digit)
}

pub fun is_ascii_whitespace: (c: Char) -> Boolean = {
    val code = c |> char_code;
    (code == 32) || (code == 9) || (code == 10) || (code == 13)
}

pub fun to_ascii_upper: (c: Char) -> Char = {
    (c |> is_ascii_lower) then {
        val shifted = ((c |> char_code) - 32) |> char_from_code;
        shifted match {
            Some(upper) => { upper }
            None => { c }
        }
    } else {
        c
    }
}

pub fun to_ascii_lower: (c: Char) -> Char = {
    (c |> is_ascii_upper) then {
        val shifted = ((c |> char_code) + 32) |> char_from_code;
        shifted match {
            Some(lower) => { lower }
            None => { c }
        }
    } else {
        c
    }
}

pub fun char_digit_value: (c: Char) -> Option<Int32> = {
    (c |> is_ascii_digit) then {
        ((c |> char_code) - 48) Option::Some
    } else {
        () Option::None
    }
}
