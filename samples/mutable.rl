// Mutable Variables & Loops
//
// Use 'mut val' to declare a mutable binding.
// Loops use the 'while' keyword with OSV syntax:
//   condition while { body }

fun main: () -> Int32 = {
    mut val i = 1;
    mut val total = 0;
    (i <= 20) while {
        total = total + i;
        i = i + 1
    }
    total
}
