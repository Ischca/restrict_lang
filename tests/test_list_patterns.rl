// Test list pattern matching

fun test_empty_list = {
    val lst = []
    lst match {
        [] => 1
        _ => 0
    }
}

fun test_exact_pattern = {
    val lst = [1, 2, 3]
    lst match {
        [] => 0
        [a] => a
        [a, b] => a + b
        [a, b, c] => a + b + c
        _ => -1
    }
}

fun test_cons_pattern = {
    val lst = [10, 20, 30]
    lst match {
        [] => 0
        [head | tail] => head + tail.list_length
    }
}

fun sum_list = lst: List<Int> {
    lst match {
        [] => 0
        [head | tail] => head + tail sum_list
    }
}

fun main = {
    with Arena {
        val empty = test_empty_list()
        val exact = test_exact_pattern()
        val cons = test_cons_pattern()
        val sum = [1, 2, 3, 4, 5] sum_list
        
        // Should print: 1 6 12 15
        empty.println
        exact.println
        cons.println
        sum.println
    }
}