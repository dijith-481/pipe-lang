// Factorial — recursive and tail-recursive

// Recursive factorial
let factorial : (i32) -> i32 = (n) => match n {
    0 => 1
    1 => 1
    n => n * factorial(n - 1)
}

// Tail-recursive with accumulator
let factorialAcc : i32 -> i32 -> i32 = (n) => (acc) => match n {
    0 => acc
    1 => acc
    n => factorialAcc(n - 1)(n * acc)
}

let factorialTail : (i32) -> i32 = (n) => factorialAcc(n)(1)

let main = () => factorialTail(5)
