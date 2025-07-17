// Generic function type parameter inference example

fun identity<T> = x: T {
    x
}

fun pair<A, B> = a: A b: B {
    record Pair<A, B> { first: A second: B }
    Pair { first: a second: b }
}

fun map<T, U> = lst: List<T> f: T -> U {
    lst match {
        [] => { [] }
        [head | tail] => {
            val new_head = (head) f;
            val new_tail = (tail, f) map;
            [new_head | new_tail]
        }
    }
}

fun main = {
    // Type parameter T is inferred as Int32
    val x = (42) identity;
    
    // Type parameter T is inferred as String
    val s = ("hello") identity;
    
    // Type parameters A and B are inferred as Int32 and String
    val p = (10, "world") pair;
    
    // Type parameters T and U are inferred from usage
    val numbers = [1, 2, 3];
    val doubled = (numbers, |n| n * 2) map;
    
    // Generic Option functions
    val maybe = 42 some;
    val nothing = None<Int32>;
    
    doubled
}