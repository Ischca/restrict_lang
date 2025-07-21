fun process<~p> = {
    with lifetime<~local> {
        val temp = 100;
        temp + 42
    }
}

fun main = {
    with lifetime<~main> {
        process()
    }
}