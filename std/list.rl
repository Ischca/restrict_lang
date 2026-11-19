// Standard Library: List
//
// Pure-Restrict list utilities complementing compiler-registered builtins
// (list_length, list_count, list_is_empty, list_get, list_head, list_tail,
// tail, list_reverse, list_prepend, list_append, list_concat, map, filter, fold).

pub record IndexedInt {
    index: Int32,
    value: Int32
}

pub fun list_any: <T>(items: List<T>, pred: T -> Boolean) -> Boolean = {
    mut val xs = items
    mut val check = pred
    val len = xs |> list_length
    mut val i = 0
    mut val found = false
    (i < len) && (found |> not) while {
        val item = (xs, i) list_get
        (item |> check) then {
            found = true
        } else {
            i = i + 1
        }
    }
    found
}

pub fun list_all: <T>(items: List<T>, pred: T -> Boolean) -> Boolean = {
    mut val xs = items
    mut val check = pred
    val len = xs |> list_length
    mut val i = 0
    mut val ok = true
    (i < len) && ok while {
        val item = (xs, i) list_get
        (item |> check) then {
            i = i + 1
        } else {
            ok = false
        }
    }
    ok
}

pub fun list_find: <T>(items: List<T>, pred: T -> Boolean) -> Option<T> = {
    mut val xs = items
    mut val check = pred
    val len = xs |> list_length
    mut val i = 0
    mut val found_idx = -1
    (i < len) && (found_idx < 0) while {
        (((xs, i) list_get) |> check) then {
            found_idx = i
        } else {
            i = i + 1
        }
    }
    found_idx >= 0 then {
        ((xs, found_idx) list_get) Option::Some
    } else {
        () Option::None
    }
}

pub fun list_take: <T>(items: List<T>, count: Int32) -> List<T> = {
    mut val xs = items
    val len = xs |> list_length
    val limit = count < 0 then {
        0
    } else {
        (count, len) min
    }
    limit <= 0 then {
        []
    } else {
        mut val acc = [(xs, 0) list_get]
        mut val i = 1
        i < limit while {
            acc = (acc, (xs, i) list_get) list_append
            i = i + 1
        }
        acc
    }
}

pub fun list_drop: <T>(items: List<T>, count: Int32) -> List<T> = {
    mut val xs = items
    val len = xs |> list_length
    val start = count < 0 then {
        0
    } else {
        (count, len) min
    }
    start >= len then {
        []
    } else {
        mut val acc = [(xs, start) list_get]
        mut val i = start + 1
        i < len while {
            acc = (acc, (xs, i) list_get) list_append
            i = i + 1
        }
        acc
    }
}

pub fun list_range: (start: Int32, end: Int32) -> List<Int32> = {
    mut val acc: List<Int32> = []
    mut val i = start
    i < end while {
        acc = (acc, i) list_append
        i = i + 1
    }
    acc
}

pub fun list_sum: (items: List<Int32>) -> Int32 = {
    (items, 0, | acc, x | acc + x) fold
}

pub fun list_sum_i64: (items: List<Int64>) -> Int64 = {
    val zero: Int64 = 0
    (items, zero, | acc, x | acc + x) fold
}

pub fun list_sum_f: (items: List<Float64>) -> Float64 = {
    (items, 0.0, | acc, x | acc + x) fold
}

pub fun list_product: (items: List<Int32>) -> Int32 = {
    (items, 1, | acc, x | acc * x) fold
}

pub fun list_enumerate_from: (items: List<Int32>, start: Int32) -> List<IndexedInt> = {
    mut val xs = items
    val len = xs |> list_length
    mut val acc: List<IndexedInt> = []
    mut val i = 0
    i < len while {
        val entry = IndexedInt {
            index: start + i,
            value: (xs, i) list_get
        }
        acc = (acc, entry) list_append
        i = i + 1
    }
    acc
}

pub fun list_zip_with: <A, B, C>(left: List<A>, right: List<B>, combine: (A, B) -> C) -> List<C> = {
    mut val xs = left
    mut val ys = right
    mut val f = combine
    val limit = (xs |> list_length, ys |> list_length) min
    limit <= 0 then {
        []
    } else {
        mut val acc = [((xs, 0) list_get, (ys, 0) list_get) f]
        mut val i = 1
        i < limit while {
            val a = (xs, i) list_get
            val b = (ys, i) list_get
            acc = (acc, (a, b) f) list_append
            i = i + 1
        }
        acc
    }
}
