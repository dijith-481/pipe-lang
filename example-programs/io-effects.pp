// IO and Effect system — pure/impure separation
// `read_line` is the only IO operation that takes/modifies state;
// `println` is in the prelude.
use stdlib::io

// Pure function — no side effects
let greet : (str) -> str = (name) => `Hello, ${name}!`

// Pure numeric conversion
let celsiusToFahrenheit : (f64) -> f64 = (c) => c * 9.0 / 5.0 + 32.0

// Effectful computation — using flat_map to chain effects
let main = () =>
    read_line()
        .flat_map((name) => {
            println(greet(name))
            println(``)
            println(`Temperature conversions:`)
            let temps = [0.0, 20.0, 37.0, 100.0]
            temps.map((c) => {
                let f = celsiusToFahrenheit(c)
                println(`${c}C = ${f}F`)
            })
        })
        .flat_map((_) => {
            println(``)
            println(`Reading from stdin...`)
            read_line()
        })
        .map((line) => println(`You said: ${line}`))
