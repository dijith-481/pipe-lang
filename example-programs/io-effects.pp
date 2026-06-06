// IO and Effect system — pure/impure separation
// `io.readLine` is the only operation that genuinely requires the `io` import;
// `println` is in the prelude. Side effects must live inside a `do` block.
use stdlib::io

// Pure function — no side effects
let greet : (str) -> str = (name) => `Hello, ${name}!`

// Pure numeric conversion
let celsiusToFahrenheit : (f64) -> f64 = (c) => c * 9.0 / 5.0 + 32.0

// Effectful computation — must be in do block
let main : () -> Effect<()> = do {
    println(`What is your name?`)
    name <- io.readLine()
    println(greet(name))

    println(``)
    println(`Temperature conversions:`)
    let temps = [0.0, 20.0, 37.0, 100.0]
    temps.map((c) => {
        let f = celsiusToFahrenheit(c)
        println(`${c}C = ${f}F`)
    })

    println(``)
    println(`Reading from stdin...`)
    line <- io.readLine()
    println(`You said: ${line}`)
}
