// Record fields use colons in declarations and literals.
record Point {
    x: Int32
    y: Int32
}

fun read_x: (point: Point) -> Int32 = {
    point.x
}

fun main: () = {
    Point { x: 42, y: 7 } read_x println
}
