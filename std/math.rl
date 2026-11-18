// Standard Library: Math
//
// Pure-Restrict numeric helpers complementing compiler-registered builtins
// (abs, max, min, pow, factorial, abs_f, max_f, min_f).

pub fun clamp: (value: Int32, low: Int32, high: Int32) -> Int32 = {
    value < low then {
        low
    } else {
        value > high then {
            high
        } else {
            value
        }
    }
}

pub fun signum: (value: Int32) -> Int32 = {
    value < 0 then {
        -1
    } else {
        value > 0 then {
            1
        } else {
            0
        }
    }
}

pub fun is_even: (value: Int32) -> Boolean = {
    (value % 2) == 0
}

pub fun is_odd: (value: Int32) -> Boolean = {
    (value % 2) != 0
}

pub fun gcd: (a: Int32, b: Int32) -> Int32 = {
    mut val x = a |> abs;
    mut val y = b |> abs;
    y != 0 while {
        val rem = x % y;
        x = y;
        y = rem
    };
    x
}

pub fun lcm: (a: Int32, b: Int32) -> Int32 = {
    (a == 0 || b == 0) then {
        0
    } else {
        val g = (a, b) gcd;
        ((a |> abs) / g) * (b |> abs)
    }
}

pub fun abs_i64: (value: Int64) -> Int64 = {
    val zero: Int64 = 0;
    value < zero then {
        zero - value
    } else {
        value
    }
}

pub fun min_i64: (left: Int64, right: Int64) -> Int64 = {
    left < right then {
        left
    } else {
        right
    }
}

pub fun max_i64: (left: Int64, right: Int64) -> Int64 = {
    left > right then {
        left
    } else {
        right
    }
}

pub fun clamp_i64: (value: Int64, low: Int64, high: Int64) -> Int64 = {
    value < low then {
        low
    } else {
        value > high then {
            high
        } else {
            value
        }
    }
}

pub fun signum_i64: (value: Int64) -> Int32 = {
    val zero: Int64 = 0;
    value < zero then {
        -1
    } else {
        value > zero then {
            1
        } else {
            0
        }
    }
}

pub fun clamp_f: (value: Float64, low: Float64, high: Float64) -> Float64 = {
    value < low then {
        low
    } else {
        value > high then {
            high
        } else {
            value
        }
    }
}

pub fun lerp_f: (start: Float64, end: Float64, t: Float64) -> Float64 = {
    start + ((end - start) * t)
}

pub fun approx_eq_f: (left: Float64, right: Float64, epsilon: Float64) -> Boolean = {
    ((left - right) |> abs_f) <= epsilon
}
