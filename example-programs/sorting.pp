// Quicksort — classic functional algorithm
// Recursive functions need explicit type signatures so HM can resolve
// the self-reference; non-recursive helpers stay inferred.

let quicksort : (Array<i32>) -> Array<i32> = (arr) => match arr.len() {
    0usize => []
    1usize => [arr[0usize]]
    _ => {
        let pivot = arr[0usize]
        let rest = arr.drop(1usize)
        let less = rest.filter((x) => x <= pivot)
        let greater = rest.filter((x) => x > pivot)
        quicksort(less).concat([pivot]).concat(quicksort(greater))
    }
}

// Merge sort — divide and conquer
let split = (arr) => {
    let mid = arr.len() / 2usize
    (arr.take(mid), arr.drop(mid))
}

let merge : (Array<i32>, Array<i32>) -> Array<i32> = (a, b) => match (a.len(), b.len()) {
    (0usize, _) => b
    (_, 0usize) => a
    _ => {
        let aHead = a[0usize]
        let bHead = b[0usize]
        if aHead <= bHead {
            [aHead].concat(merge(a.drop(1usize), b))
        } else {
            [bHead].concat(merge(a, b.drop(1usize)))
        }
    }
}

let mergesort : (Array<i32>) -> Array<i32> = (arr) => match arr.len() {
    0usize => []
    1usize => [arr[0usize]]
    _ => {
        let (left, right) = split(arr)
        merge(mergesort(left), mergesort(right))
    }
}

let main = () => {
    let data = [38, 27, 43, 3, 9, 82, 10, 55, 12, 1]
    println(`Original:  ${data}`)
    println(`Quicksort: ${quicksort(data)}`)
    println(`Mergesort: ${mergesort(data)}`)
}
