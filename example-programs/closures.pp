// Closures and lexical scope
use stdlib.io

// Closure capturing environment
let makeAdder : (i32) -> (i32) -> i32
let makeAdder = (n) => (x) => x + n

let add5 : (i32) -> i32
let add5 = makeAdder(5)

let add10 : (i32) -> i32
let add10 = makeAdder(10)

// Closure in higher-order function
let applyTwice : ((i32) -> i32, i32) -> i32
let applyTwice = (f, x) => f(f(x))

// Counter using block closure
let makeCounter : () -> () -> i32
let makeCounter = () => {
    let count = [0]  // mutable cell via array
    () => {
        let current = count[0]
        count[0] = current + 1
        current
    }
}

// Function composition via closures
let compose : ((i32) -> i32, (i32) -> i32) -> (i32) -> i32
let compose = (f, g) => (x) => f(g(x))

let double : (i32) -> i32
let double = (x) => x * 2

let increment : (i32) -> i32
let increment = (x) => x + 1

let doubleThenIncrement : (i32) -> i32
let doubleThenIncrement = compose(increment, double)

let main : () -> Effect<Unit>
let main = do {
    IO.println("add5(10) = " ++ add5(10).toString())
    IO.println("add10(10) = " ++ add10(10).toString())
    IO.println("applyTwice(double, 3) = " ++ applyTwice(double, 3).toString())
    IO.println("doubleThenIncrement(5) = " ++ doubleThenIncrement(5).toString())
}
