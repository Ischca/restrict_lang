// println accepts any type with a Display adoption.
record Notice {
    text: String
}

Notice takes Display {
    fun display: (self: Notice) -> String = {
        self.text
    }
}

fun main: () -> () = {
    42 |> println
    "built-in String" |> println
    Notice { text: "record adoption" } |> println
}
