// IO and Effect system — pure/impure separation
// `println` is in the prelude.
// `io.read_line` requires `use stdlib::io`.

// Pure function — no side effects
let greet : (str) -> str = (name) => `Hello, ${name}!`

// Pure numeric conversion
let celsius_to_fahrenheit : (f64) -> f64 = (c) => c * 9.0 / 5.0 + 32.0

// Effectful computation — using flatMap to chain effects
let main = () => {
    println(`Temperature conversions:`)
    let temps = [0.0, 20.0, 37.0, 100.0]
    temps.map((c) => {
        let f = celsius_to_fahrenheit(c)
        println(`${c}C = ${f}F`)
    })
    println(``)
    println(`Done.`)
}
