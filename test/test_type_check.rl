record Point { x: Int y: Int }

val p1 = Point { x = 10, y = 20 }
val p2 = p1.clone { x = 30 }
val p3 = p2 freeze

val x = p3.x
val y = p3.y