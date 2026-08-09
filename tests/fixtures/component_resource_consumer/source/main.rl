pub record Counter {
    value: Int32
}

fun close_counter: (counter: Counter) -> () = {
    ()
}

fun read_counter: (counter: Counter) -> Int32 = {
    counter.value
}

fun finish_counter: (counter: Counter) -> Int32 = {
    counter.value
}

pub fun make_counter: (value: Int32) -> Counter = {
    Counter { value: value }
}

pub fun make_transform: (offset: Int32) -> Int32 -> Int32 = {
    |value: Int32| value + offset
}
