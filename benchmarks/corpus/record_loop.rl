record Pair {
    left: Int32
    right: Int32
}

fun pair_score: (pair: Pair) -> Int32 = {
    pair.left * 31 + pair.right
}

pub fun benchmark: (iterations: Int32) -> Int32 = {
    mut val index = 0;
    mut val checksum = 0;
    (index < iterations) while {
        val pair = Pair { left: index, right: index + 1 };
        checksum = checksum + (pair pair_score);
        index = index + 1
    }
    checksum
}
