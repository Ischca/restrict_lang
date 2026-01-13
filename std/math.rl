// Restrict Language Standard Library: Math Functions
// 標準ライブラリ: 数学関数

// ============================================================
// Absolute Value
// ============================================================

// Get absolute value of an integer
export fun abs: (x: Int) -> Int = {
    x < 0 then { 0 - x } else { x }
}

// ============================================================
// Min/Max Functions
// ============================================================

// Return the smaller of two integers
export fun min: (a: Int, b: Int) -> Int = {
    a < b then { a } else { b }
}

// Return the larger of two integers
export fun max: (a: Int, b: Int) -> Int = {
    a > b then { a } else { b }
}

// ============================================================
// Sign Functions
// ============================================================

// Return the sign of an integer: -1, 0, or 1
export fun signum: (x: Int) -> Int = {
    x < 0 then { 0 - 1 } else { x > 0 then { 1 } else { 0 } }
}

// Check if a number is positive
export fun is_positive: (x: Int) -> Bool = {
    x > 0
}

// Check if a number is negative
export fun is_negative: (x: Int) -> Bool = {
    x < 0
}

// Check if a number is zero
export fun is_zero: (x: Int) -> Bool = {
    x == 0
}

// ============================================================
// Integer Division Functions
// ============================================================

// Integer power: base^exp (for non-negative exponents)
export fun pow: (base: Int, exp: Int) -> Int = {
    exp == 0 then { 1 } else {
        exp == 1 then { base } else {
            val half = (base, exp / 2) pow
            (exp % 2) == 0 then {
                half * half
            } else {
                half * half * base
            }
        }
    }
}

// Greatest common divisor (Euclidean algorithm)
export fun gcd: (a: Int, b: Int) -> Int = {
    b == 0 then { a abs } else {
        (b, a % b) gcd
    }
}

// Least common multiple
export fun lcm: (a: Int, b: Int) -> Int = {
    val g = (a, b) gcd
    g == 0 then { 0 } else {
        (a abs) / g * (b abs)
    }
}

// ============================================================
// Clamping
// ============================================================

// Clamp a value to a range [lo, hi]
export fun clamp: (x: Int, lo: Int, hi: Int) -> Int = {
    x < lo then { lo } else {
        x > hi then { hi } else { x }
    }
}
