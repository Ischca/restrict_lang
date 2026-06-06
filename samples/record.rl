// Records & Arena Memory
//
// Records are value types with named fields.
// Field initializers use ':' and OSV calls keep values flowing left-to-right.

record Point { x: Int32, y: Int32 }

fun show_x: (p: Point) -> Int32 = {
    p.x
}

fun main: () -> Int32 = {
    val p = Point { x: 3, y: 4 }
    p |> show_x
}
