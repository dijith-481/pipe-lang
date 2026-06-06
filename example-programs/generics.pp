// Type signatures and generic functions

// Polymorphic identity function
let id : (a) -> a = (x) => x

// Constant function
let const : (a) -> (b) -> a = (a) => (b) => a

// Flip argument order
let flip : ((a, b) -> c) -> (b, a) -> c = (f) => (b, a) => f(a, b)

// Compose two functions
let compose : ((b) -> c, (a) -> b) -> (a) -> c = (f, g) => (x) => f(g(x))

// Pipe (reverse compose)
let pipe : ((a) -> b, (b) -> c) -> (a) -> c = (f, g) => (x) => g(f(x))

// Apply a function to a value (useful for chaining)
let apply : ((a) -> b, a) -> b = (f, x) => f(x)

// Type signatures with generics
let mapOption = (opt, f) => match opt {
    None    => None
    Some(x) => Some(f(x))
}

let mapResult = (result, f) => match result {
    Err(e)  => Err(e)
    Ok(v)   => Ok(f(v))
}

let main : () -> Effect<()> = do {
    // Using generic functions
    let x = id(42)
    let s = id("hello")
    println("id(42) = " ++ x.toString())
    println("id(\"hello\") = " ++ s)

    // Compose
    let double = (n) => n * 2
    let increment = (n) => n + 1
    let doubleThenInc = compose(increment, double)
    println("doubleThenInc(5) = " ++ doubleThenInc(5).toString())

    // Map over Option
    let opt = Some(10)
    let mapped = mapOption(opt, (n) => n * 3)
    println("mapOption(Some(10), *3) = " ++ mapped.toString())

    // Map over Result
    let res = Ok(42)
    let mappedRes = mapResult(res, (n) => n - 10)
    println("mapResult(Ok(42), -10) = " ++ mappedRes.toString())
}
