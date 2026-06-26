// Option and Result — error handling without exceptions
// `Some`, `None`, `Ok`, `Err`, `Option<T>`, `Result<T, E>`, and their methods
// (`.map`, `.unwrapOr`, `.flatMap`, ...) are all in the prelude.

// Simulated database lookup
let find_user = (id) => match id {
    1 => Some({ id: 1, name: `Alice`,   email: `alice@example.com` })
    2 => Some({ id: 2, name: `Bob`,     email: `bob@example.com` })
    3 => Some({ id: 3, name: `Charlie`, email: `charlie@example.com` })
    _ => None
}

// Parse a string to i32 (simplified)
let parse_age = (s) => match s {
    "0"  => Ok(0)
    "1"  => Ok(1)
    "2"  => Ok(2)
    "18" => Ok(18)
    "25" => Ok(25)
    _    => Err(`invalid age: ${s}`)
}

// Process user if found
let greet_user = (user_id) =>
    find_user(user_id)
        .map((user: {id: i32, name: str, email: str}) => `Hello, ${user.name}!`)

// Validate age
let validate_age = (input) =>
    match parse_age(input) {
        Ok(age) => if age >= 0 { if age <= 150 { Ok(age) } else { Err(`age out of range`) } } else { Err(`age out of range`) }
        Err(e) => Err(e)
    }

let main = () => {
    // Option usage
    println(`--- Option ---`)
    println(greet_user(1).unwrap_or(`User not found`))
    println(greet_user(99).unwrap_or(`User not found`))

    // Result usage
    println(`--- Result ---`)
    let age1 = validate_age(`25`)
    let age2 = validate_age(`abc`)
    let age3 = validate_age(`200`)

    println(age1.map((age) => `${age}`).unwrap_or(`error`))
    println(age2.map((age) => `${age}`).unwrap_or(`error`))
    println(age3.map((age) => `${age}`).unwrap_or(`error`))
}
