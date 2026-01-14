fun fizzbuzz: (n: Int) = {
    n % 15 == 0 then { "FizzBuzz" } else {
        n % 3 == 0 then { "Fizz" } else {
            n % 5 == 0 then { "Buzz" } else { n int_to_string }
        }
    }
}

fun main: () = {
    mut val i = 1;
    i <= 100 while {
        i fizzbuzz |> println;
        i = i + 1
    };
}
