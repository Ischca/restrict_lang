pub fun benchmark: (iterations: Int32) -> Int32 = {
    mut val index = 0;
    mut val checksum = 0;
    (index < iterations) while {
        val shifted = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16,
            17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32] map { |value|
            value + 1
        }
        val kept = shifted filter { |value|
            value % 2 == 0
        }
        checksum = (kept, checksum) fold { |total, value|
            total + value
        };
        index = index + 1
    }
    checksum
}
