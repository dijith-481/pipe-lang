// Higher-order functions — map, filter, fold

// Double all elements
let doubleAll = (xs) => xs.map((x) => x * 2)

// Keep only even numbers
let evens = (xs) => xs.filter((x) => x % 2 == 0)

// Sum all elements
let sum = (xs) => xs.fold(0, (acc, x) => acc + x)

// Product of all elements
let product = (xs) => xs.fold(1i64, (acc, x) => acc * x)

// Find maximum
let max = (xs) => xs.fold(None, (acc, x) => match acc {
    None    => Some(x)
    Some(m) => if x > m then Some(x) else Some(m)
})

// Chain operations: sum of squares of even numbers
let sumOfSquaresOfEvens = (xs) =>
    xs
        .filter((x) => x % 2 == 0)
        .map((x) => x * x)
        .fold(0, (acc, x) => acc + x)

let main : () -> Effect<()> = do {
    let data = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
    println(`Original: ${data}`)
    println(`Doubled:  ${doubleAll(data)}`)
    println(`Evens:    ${evens(data)}`)
    println(`Sum:      ${sum(data)}`)
    println(`Max:      ${max(data)}`)
    println(`Sum of squares of evens: ${sumOfSquaresOfEvens(data)}`)
}
