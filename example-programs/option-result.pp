// Option and Result — error handling without exceptions
// `Some`, `None`, `Ok`, `Err`, `Option<T>`, `Result<T, E>`, and their methods
// (`.map`, `.unwrap`, `.flatMap`, ...) are all in the prelude.

type User = {
    id    : i32
    name  : str
    email : str
}

// Simulated database lookup
let findUser = (id) => match id {
    1 => Some({ id: 1, name: `Alice`,   email: `alice@example.com` })
    2 => Some({ id: 2, name: `Bob`,     email: `bob@example.com` })
    3 => Some({ id: 3, name: `Charlie`, email: `charlie@example.com` })
    _ => None
}

// Parse a string to i32 (simplified)
let parseAge = (s) => match s {
    `0`  => Ok(0)
    `1`  => Ok(1)
    `2`  => Ok(2)
    `18` => Ok(18)
    `25` => Ok(25)
    _    => Err(`invalid age: ${s}`)
}

// Process user if found
let greetUser = (userId) =>
    findUser(userId)
        .map((user) => `Hello, ${user.name}!`)

// Validate age
let validateAge = (input) =>
    parseAge(input)
        .flatMap((age) => if age >= 0 && age <= 150 {
            Ok(age)
        } else {
            Err(`age out of range`)
        })

let main : () -> Effect<()> = do {
    // Option usage
    println(`--- Option ---`)
    println(greetUser(1).unwrap(`User not found`))
    println(greetUser(99).unwrap(`User not found`))

    // Result usage
    println(`--- Result ---`)
    let age1 = validateAge(`25`)
    let age2 = validateAge(`abc`)
    let age3 = validateAge(`200`)

    println(age1.unwrap(`error`))
    println(age2.unwrap(`error`))
    println(age3.unwrap(`error`))
}
