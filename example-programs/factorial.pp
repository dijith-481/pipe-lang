// Factorial — recursive and iterative

// Recursive factorial
let factorial : (i32) -> i64 = (n) => match n {
    0 => 1i64
    1 => 1i64
    n => n * factorial(n - 1)
}

// Tail-recursive with accumulator
let factorialAcc : (i32, i64) -> i64 = (n, acc) => match n {
    0 => acc
    1 => acc
    n => factorialAcc(n - 1, n * acc)
}

let factorialTail : (i32) -> i64 = (n) => factorialAcc(n, 1i64)

// Compute factorials of 0 through 10
let main : () -> Effect<()> = do {
    let nums = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
    nums.map((n) => {
        let result = factorialTail(n)
        println(n.toString() ++ "! = " ++ result.toString())
    })
}
