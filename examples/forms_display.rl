// The compiler supplies Display for scalar values.
// User records adopt it explicitly.
record Notice {
    text: String
}

Notice takes Display {
    fun display: (self: Notice) -> String = {
        self.text
    }
}

fun main: () -> () = {
    42 |> print
    " · " |> print
    Notice { text: "records too" } |> println
}
