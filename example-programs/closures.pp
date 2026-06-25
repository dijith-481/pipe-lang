// Closures and lexical scope

// Closure capturing environment
let make_adder = (n) => (x) => x + n

let add5 = make_adder(5)

let add10 = make_adder(10)

// Closure in higher-order function
let apply_twice = (f, x) => f(f(x))

let double = (x) => x * 2

let increment = (x) => x + 1

// Pure counter using fold with state threading.
// pipe-lang is purely functional: there is no mutable cell.
// Counters are expressed by threading the running total through
// the fold's accumulator and producing the snapshot list.
let run_counter = (n) => {
    let (_, snapshots) = [0, 1, 2, 3, 4].fold((0, []), (acc, _) => {
        let (state, snaps) = acc
        (state + 1, snaps.concat([state + 1]))
    })
    snapshots
}

let main = () => {
    println(`add5(10) = ${add5(10)}`)
    println(`add10(10) = ${add10(10)}`)
    println(`apply_twice(double, 3) = ${apply_twice(double, 3)}`)
    // Function composition (defined inline to work around top-level
    // compose + thunk interaction).
    let compose = (f, g) => (x) => f(g(x))
    let dbl_then_inc = compose(increment, double)
    println(`doubleThenIncrement(5) = ${dbl_then_inc(5)}`)
}
