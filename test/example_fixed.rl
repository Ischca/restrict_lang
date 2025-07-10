record Point { x: Int y: Int }

fun makePoint = x: Int y: Int {
    Point { x = x, y = y }
}

val p1 = (10, 20) makePoint
val p2 = p1.clone { x = 30 }
val p3 = p2 freeze