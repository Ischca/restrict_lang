// Standard Library: Prelude
//
// Core compiler-registered builtins (identity, map, filter, fold, not, and, or,
// assert, panic) are automatically available without importing. This module
// exports pure-Restrict utility helpers that complement those builtins.

pub fun identity_val: <T>(value: T) -> T = {
    value
}

pub fun bool_xor: (left: Boolean, right: Boolean) -> Boolean = {
    (left && (right |> not)) || ((left |> not) && right)
}

pub fun bool_implies: (condition: Boolean, consequent: Boolean) -> Boolean = {
    condition then {
        consequent
    } else {
        true
    }
}

pub fun bool_to_int: (flag: Boolean) -> Int32 = {
    flag then {
        1
    } else {
        0
    }
}

pub fun eq_int: (left: Int32, right: Int32) -> Boolean = {
    left == right
}

pub fun ne_int: (left: Int32, right: Int32) -> Boolean = {
    left != right
}

pub fun cmp_int: (left: Int32, right: Int32) -> Int32 = {
    left < right then {
        -1
    } else {
        left > right then {
            1
        } else {
            0
        }
    }
}

pub fun cmp_int64: (left: Int64, right: Int64) -> Int32 = {
    left < right then {
        -1
    } else {
        left > right then {
            1
        } else {
            0
        }
    }
}

pub fun cmp_float: (left: Float64, right: Float64) -> Int32 = {
    left < right then {
        -1
    } else {
        left > right then {
            1
        } else {
            0
        }
    }
}
