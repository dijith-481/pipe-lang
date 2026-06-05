// Type signatures and generic functions
use stdlib.io

// Polymorphic identity function
let id : (a) -> a
let id = (x) => x

// Constant function
let const : (a) -> (b) -> a
let const = (a) => (b) => a

// Flip argument order
let flip : ((a, b) -> c) -> (b, a) -> c
let flip = (f) => (b, a) => f(a, b)

// Compose two functions
let compose : ((b) -> c, (a) -> b) -> (a) -> c
let compose = (f, g) => (x) => f(g(x))

// Pipe (reverse compose)
let pipe : ((a) -> b, (b) -> c) -> (a) -> c
let pipe = (f, g) => (x) => g(f(x))

// Apply a function to a value (useful for chaining)
let apply : ((a) -> b, a) -> b
let apply = (f, x) => f(x)

// Type signatures with generics
let mapOption : <A, B>(Option<A>, (A) -> B) -> Option<B>
let mapOption = (opt, f) => match opt {
    None    => None
    Some(x) => Some(f(x))
}

let mapResult : <T, E, U>(Result<T, E>, (T) -> U) -> Result<U, E>
let mapResult = (result, f) => match result {
    Err(e)  => Err(e)
    Ok(v)   => Ok(f(v))
}

let main : () -> Effect<Unit>
let main = do {
    // Using generic functions
    let x = id(42)
    let s = id("hello")
    IO.println("id(42) = " ++ x.toString())
    IO.println("id(\"hello\") = " ++ s)

    // Compose
    let double = (n) => n * 2
    let increment = (n) => n + 1
    let doubleThenInc = compose(increment, double)
    IO.println("doubleThenInc(5) = " ++ doubleThenInc(5).toString())

    // Map over Option
    let opt = Some(10)
    let mapped = mapOption(opt, (n) => n * 3)
    IO.println("mapOption(Some(10), *3) = " ++ mapped.toString())

    // Map over Result
    let res = Ok(42)
    let mappedRes = mapResult(res, (n) => n - 10)
    IO.println("mapResult(Ok(42), -10) = " ++ mappedRes.toString())
}
