// Type signatures and generic functions

// Polymorphic identity function (explicit for pedagogy)
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
let map_option = (opt, f) => match opt {
    None    => None
    Some(x) => Some(f(x))
}

let map_result = (result, f) => match result {
    Err(e)  => Err(e)
    Ok(v)   => Ok(f(v))
}

let get_value= (result) => match result {
    Err(e)  => 0
    Ok(v)   => v
}

let main = () => {
    // Using generic functions
    let x = id(42)
    let s = id(`hello`)
    println(`id(42) = ${x}`)
    println(`id("hello") = ${s}`)

    // Compose
    let double = (n) => n * 2
    let increment = (n) => n + 1
    let double_then_inc = compose(increment, double)
    println(`double_then_inc(5) = ${double_then_inc(5)}`)

    // Map over Option
    let opt = Some(10)
    let mapped = map_option(opt, (n) => n * 3)
    println(`map_option(Some(10), *3) = ${mapped.unwrap_or(0)}`)

    // Map over Result
    let res = Ok(42)
    let mapped_res = map_result(res, (n) => n - 10)
    println(`map_result(Ok(42), -10) = ${get_value(mapped_res)}`)
}
