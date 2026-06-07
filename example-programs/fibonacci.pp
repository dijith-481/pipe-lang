// Fibonacci — recursive and tail-recursive

// Recursive (naive)
let fib : (i32) -> i64 = (n) => match n {
    0 => 0i64
    1 => 1i64
    n => fib(n - 1) + fib(n - 2)
}

// Tail-recursive helper
let fibTail : (i32, i64, i64) -> i64 = (n, a, b) => match n {
    0 => a
    n => fibTail(n - 1, b, a + b)
}

// Optimized entry point
let fibFast : (i32) -> i64 = (n) => fibTail(n, 0i64, 1i64)

// Print first 20 Fibonacci numbers
let main = () =>
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19]
        .map((n) => fibFast(n))
        .map((v) => println(`${v}`))
