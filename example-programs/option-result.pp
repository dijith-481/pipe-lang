// Option and Result — error handling without exceptions
use stdlib.io

type User = {
    id    : i32
    name  : str
    email : str
}

// Simulated database lookup
let findUser : (i32) -> Option<User>
let findUser = (id) => match id {
    1 => Some({ id: 1, name: "Alice", email: "alice@example.com" })
    2 => Some({ id: 2, name: "Bob",   email: "bob@example.com" })
    3 => Some({ id: 3, name: "Charlie", email: "charlie@example.com" })
    _ => None
}

// Parse a string to i32 (simplified)
let parseAge : (str) -> Result<i32, str>
let parseAge = (s) => match s {
    "0"  => Ok(0)
    "1"  => Ok(1)
    "2"  => Ok(2)
    "18" => Ok(18)
    "25" => Ok(25)
    _    => Err("invalid age: " ++ s)
}

// Process user if found
let greetUser : (i32) -> Option<str>
let greetUser = (userId) =>
    findUser(userId)
        .map((user) => "Hello, " ++ user.name + "!")

// Validate age
let validateAge : (str) -> Result<i32, str>
let validateAge = (input) =>
    parseAge(input)
        .flatMap((age) => if age >= 0 && age <= 150 {
            Ok(age)
        } else {
            Err("age out of range")
        })

let main : () -> Effect<Unit>
let main = do {
    // Option usage
    IO.println("--- Option ---")
    IO.println(greetUser(1).unwrap("User not found"))
    IO.println(greetUser(99).unwrap("User not found"))

    // Result usage
    IO.println("--- Result ---")
    let age1 = validateAge("25")
    let age2 = validateAge("abc")
    let age3 = validateAge("200")

    IO.println(age1.unwrap("error"))
    IO.println(age2.unwrap("error"))
    IO.println(age3.unwrap("error"))
}
