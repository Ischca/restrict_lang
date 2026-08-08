pub fun benchmark: (iterations: Int32) -> Int32 = {
    mut val index = 0;
    mut val checksum = 0;
    (index < iterations) while {
        checksum = checksum + (index * 3 + 1);
        index = index + 1
    }
    checksum
}
