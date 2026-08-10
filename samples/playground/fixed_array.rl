// Array carries its exact length in the source type.
fun main: () = {
    val readings: Array<Int32, 3> = [10, 20, 30];
    (readings, 1) array_get println
}
