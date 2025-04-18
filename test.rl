{
    fun fizzBuzz = n: Int {
        fun helper = i: Int {
            if i > n then {}
            else {
                if i % 15 == 0 then "FizzBuzz"
                else if i % 5 == 0 then "Buzz"
                else if i % 3 == 0 then "Fizz"
                else i
                |> println

                i + 1 helper
            }
        }

        1 helper
    }

    20 fizzBuzz
}