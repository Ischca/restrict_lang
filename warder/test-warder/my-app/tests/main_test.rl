// Example executable test

import project.main.{answer}

pub fun test_example: () -> Boolean = {
    "checking the project import" println;
    () answer == 42
}
