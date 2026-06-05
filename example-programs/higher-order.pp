// Higher-order functions — map, filter, fold
use stdlib.io

// Double all elements
let doubleAll : (Array<i32>) -> Array<i32>
let doubleAll = (xs) => xs.map((x) => x * 2)

// Keep only even numbers
let evens : (Array<i32>) -> Array<i32>
let evens = (xs) => xs.filter((x) => x % 2 == 0)

// Sum all elements
let sum : (Array<i32>) -> i32
let sum = (xs) => xs.fold(0, (acc, x) => acc + x)

// Product of all elements
let product : (Array<i32>) -> i64
let product = (xs) => xs.fold(1i64, (acc, x) => acc * x)

// Find maximum
let max : (Array<i32>) -> Option<i32>
let max = (xs) => xs.fold(None, (acc, x) => match acc {
    None    => Some(x)
    Some(m) => if x > m then Some(x) else Some(m)
})

// Chain operations: sum of squares of even numbers
let sumOfSquaresOfEvens : (Array<i32>) -> i32
let sumOfSquaresOfEvens = (xs) =>
    xs
        .filter((x) => x % 2 == 0)
        .map((x) => x * x)
        .fold(0, (acc, x) => acc + x)

let main : () -> Effect<Unit>
let main = do {
    let data = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
    IO.println("Original: " ++ data.toString())
    IO.println("Doubled:  " ++ doubleAll(data).toString())
    IO.println("Evens:    " ++ evens(data).toString())
    IO.println("Sum:      " ++ sum(data).toString())
    IO.println("Max:      " ++ max(data).toString())
    IO.println("Sum of squares of evens: " ++ sumOfSquaresOfEvens(data).toString())
}
