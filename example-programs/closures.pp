// Closures and lexical scope

// Closure capturing environment
let makeAdder = (n) => (x) => x + n

let add5 = makeAdder(5)

let add10 = makeAdder(10)

// Closure in higher-order function
let applyTwice = (f, x) => f(f(x))

// Function composition via closures (type inferred by HM)
let compose = (f, g) => (x) => f(g(x))

let double = (x) => x * 2

let increment = (x) => x + 1

let doubleThenIncrement = compose(increment, double)

// Pure counter using fold with state threading.
// pipe-lang is purely functional: there is no mutable cell.
// Counters are expressed by threading the running total through
// the fold's accumulator and producing the snapshot list.
let runCounter = (n) => {
    let (_, snapshots) = [0, 1, 2, 3, 4].fold((0, []), (acc, _) => {
        let (state, snaps) = acc
        (state + 1, snaps.concat([state + 1]))
    })
    snapshots
}

let main = () => {
    println(`add5(10) = ${add5(10)}`)
    println(`add10(10) = ${add10(10)}`)
    println(`applyTwice(double, 3) = ${applyTwice(double, 3)}`)
    println(`doubleThenIncrement(5) = ${doubleThenIncrement(5)}`)
    println(`runCounter(5) = ${runCounter(5)}`)
}
