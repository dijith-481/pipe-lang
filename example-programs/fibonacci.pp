// Fibonacci — naive recursive (exponential time, linear stack)

let fib = (n) => match n {
    0 => 0
    1 => 1
    n => fib(n - 1) + fib(n - 2)
}

let main = () => {
println(`${fib(10)}`)
}
