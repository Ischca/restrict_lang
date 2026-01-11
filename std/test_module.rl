// Test module for import testing
// std/test_module.rl

// Exported function
export fun double: (x: Int) -> Int = {
    x * 2
}

// Exported function
export fun triple: (x: Int) -> Int = {
    x * 3
}

// Not exported (private)
fun helper: (x: Int) -> Int = {
    x + 1
}
