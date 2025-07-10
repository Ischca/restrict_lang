// Record definitions
record Point { x: Int y: Int }
record Enemy { hp: Int atk: Int name: String }

// Functions
fun makePoint = x: Int y: Int {
    Point { x = x, y = y }
}

fun distance = p: Point {
    p.x
}

// Basic values and cloning
val origin = Point { x = 0, y = 0 }
val p1 = origin.clone { x = 10 }
val p2 = p1.clone { y = 20 } freeze

// Function calls
val p3 = (5, 15) makePoint
val dist = p3 distance

// Complex record with freeze
val goblin = Enemy { hp = 50, atk = 5, name = "Goblin" }
val orc = goblin.clone { hp = 100, atk = 10 } freeze