// Higher-order functions — map, filter, fold

// Double all elements
let double_all = (xs) => xs.map((x) => x * 2)

// Keep only even numbers
let evens = (xs) => xs.filter((x) => x % 2 == 0)

// Sum all elements
let sum = (xs) => xs.fold(0, (acc, x) => acc + x)

// Product of all elements
let product = (xs) => xs.fold(1i64, (acc, x) => acc * x)

// Find maximum
let max = (xs) => xs.fold(None, (acc, x) => match acc {
    None    => Some(x)
    Some(m) => if x > m { Some(x) } else { Some(m) }
}).unwrap_or(0)

// Chain operations: sum of squares of even numbers
let sum_of_squares_of_evens = (xs) =>
    xs
        .filter((x) => x % 2 == 0)
        .map((x) => x * x)
        .fold(0, (acc, x) => acc + x)

let main = () => {
    let data = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
    data.map((x) => println(`Element: ${x}`))
    double_all(data).map((x) => println(`Doubled: ${x}`))
    evens(data).map((x) => println(`Evens:   ${x}`))
    println(`Sum:      ${sum(data)}`)
    println(`Max:      ${max(data)}`)
    println(`Sum of squares of evens: ${sum_of_squares_of_evens(data)}`)
}
