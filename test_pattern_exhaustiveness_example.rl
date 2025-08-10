// Test file for pattern matching exhaustiveness
// This file demonstrates various exhaustiveness scenarios

// Test 1: Boolean exhaustiveness - missing case
fun test_boolean_incomplete(flag: Boolean) -> String {
    flag |> match {
        true => "yes"
        // Missing false case - should trigger exhaustiveness error
    }
}

// Test 2: Boolean exhaustiveness - complete
fun test_boolean_complete(flag: Boolean) -> String {
    flag |> match {
        true => "yes",
        false => "no"
    }
}

// Test 3: Option exhaustiveness - missing None
fun test_option_incomplete(maybe: Option<Int32>) -> Int32 {
    maybe |> match {
        Some(value) => value
        // Missing None case - should trigger exhaustiveness error  
    }
}

// Test 4: Option exhaustiveness - complete
fun test_option_complete(maybe: Option<Int32>) -> Int32 {
    maybe |> match {
        Some(value) => value,
        None => 0
    }
}

// Test 5: List exhaustiveness - missing empty list
fun test_list_incomplete(items: List<Int32>) -> Int32 {
    items |> match {
        [head | tail] => head
        // Missing [] case - should trigger exhaustiveness error
    }
}

// Test 6: List exhaustiveness - complete  
fun test_list_complete(items: List<Int32>) -> Int32 {
    items |> match {
        [] => 0,
        [head | tail] => head
    }
}

// Test 7: Nested patterns - incomplete
fun test_nested_incomplete(nested: Option<Option<Int32>>) -> Int32 {
    nested |> match {
        Some(Some(value)) => value,
        None => 0
        // Missing Some(None) case - should trigger exhaustiveness error
    }
}

// Test 8: Wildcard makes everything exhaustive
fun test_wildcard_exhaustive(value: Int32) -> String {
    value |> match {
        42 => "answer",
        _ => "other"
    }
}