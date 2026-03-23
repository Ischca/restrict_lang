// Records & Arena Memory
//
// Records are value types with named fields.
// Heap allocation happens inside 'with Arena { ... }' blocks,
// ensuring memory is freed when the scope ends.
// Use .clone {} to create a copy — the original is consumed
// by clone (affine semantics).

record Point { x: Int, y: Int }

fun show_x: (p: Point) -> String = {
    p.x int_to_string
}

fun main = {
    with Arena {
        val p = Point { x = 3, y = 4 }
        p show_x |> println                // consumes p => "3"

        val q = Point { x = 10, y = 20 }
        val q2 = q.clone {}                // consumes q, returns a copy
        q2.y int_to_string |> println      // consumes q2 => "20"
    }
    // Arena memory is freed here
}
