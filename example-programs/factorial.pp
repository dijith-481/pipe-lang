// Factorial — recursive

// Recursive factorial
let factorial : (u64) -> u64 = (n) => match n {
    0u64 => 1u64
    1u64 => 1u64
    n => n * factorial(n - 1u64)
}

let main = () => {
println(`${factorial(5u64)}`)
}
