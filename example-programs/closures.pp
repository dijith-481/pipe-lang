// Closures and lexical scope

// Closure capturing environment
let makeAdder = (n) => (x) => x + n

let add5 = makeAdder(5)

let add10 = makeAdder(10)

// Closure in higher-order function
let applyTwice = (f, x) => f(f(x))

// Counter using block closure (array as a mutable cell — pipe-lang is pure by default)
let makeCounter = () => {
    let count = [0]
    () => {
        let current = count[0]
        count[0] = current + 1
        current
    }
}

// Function composition via closures (type inferred by HM)
let compose = (f, g) => (x) => f(g(x))

let double = (x) => x * 2

let increment = (x) => x + 1

let doubleThenIncrement = compose(increment, double)

let main : () -> Effect<()> = do {
    println("add5(10) = " ++ add5(10).toString())
    println("add10(10) = " ++ add10(10).toString())
    println("applyTwice(double, 3) = " ++ applyTwice(double, 3).toString())
    println("doubleThenIncrement(5) = " ++ doubleThenIncrement(5).toString())
}
