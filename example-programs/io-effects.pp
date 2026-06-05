// IO and Effect system — pure/impure separation
use stdlib.io

// Pure function — no side effects
let greet : (str) -> str
let greet = (name) => "Hello, " ++ name + "!"

// Pure computation
let celsiusToFahrenheit : (f64) -> f64
let celsiusToFahrenheit = (c) => c * 9.0 / 5.0 + 32.0

// Effectful computation — must be in do block
let main : () -> Effect<Unit>
let main = do {
    IO.println("What is your name?")
    name <- IO.readLine()
    IO.println(greet(name))

    IO.println("")
    IO.println("Temperature conversions:")
    let temps = [0.0, 20.0, 37.0, 100.0]
    temps.map((c) => {
        let f = celsiusToFahrenheit(c)
        IO.println(c.toString() ++ "C = " ++ f.toString() ++ "F")
    })

    IO.println("")
    IO.println("Reading from stdin...")
    line <- IO.readLine()
    IO.println("You said: " ++ line)
}
