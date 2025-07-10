// Test 1: Affine type violation - using variable twice
val x = 42
val y = x
val z = x  // Error: x already used

// Test 2: Field access consumes record
record Point { x: Int y: Int }
val p = Point { x = 10, y = 20 }
val x = p.x
val y = p.y  // Error: p already used

// Test 3: Clone allows reuse
record Enemy { hp: Int atk: Int }
val base = Enemy { hp = 100, atk = 10 }
val enemy1 = base.clone { hp = 150 }
val enemy2 = base.clone { hp = 200 }  // Error: base already used

// Test 4: Pipe operations
val result = 42 |> add 10 |> multiply 2

// Test 5: Mutable bindings
mut val counter = 0
counter = 1
counter = 2  // OK: mutable binding

// Test 6: Immutable reassignment
val fixed = 100
fixed = 200  // Error: cannot reassign immutable

// Test 7: Function parameters are consumed
fun use_point = p: Point {
    val x = p.x
    val y = p.y  // Error: p already used
}

// Test 8: Block scoping
val outer = 42
{
    val inner = outer
    val x = inner
}
val y = outer  // Error: outer already used in inner scope