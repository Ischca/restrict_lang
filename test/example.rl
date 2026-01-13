record Point { x: Int, y: Int }

fun distance = p1: Point p2: Point {
    val dx = p1.x
    val dy = p1.y
    val result = dx
}

val origin = Point { x = 0, y = 0 }
val p1 = Point { x = 3, y = 4 }
val dist = (origin, p1) distance