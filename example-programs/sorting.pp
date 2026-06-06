// Quicksort — classic functional algorithm
// Recursive functions need explicit type signatures so HM can resolve
// the self-reference; non-recursive helpers stay inferred.

let quicksort : (Array<i32>) -> Array<i32> = (arr) => match arr {
    []         => []
    [pivot]    => [pivot]
    arr        => {
        let pivot = arr[0]
        let rest = arr.drop(1)
        let less = rest.filter((x) => x <= pivot)
        let greater = rest.filter((x) => x > pivot)
        quicksort(less).concat([pivot]).concat(quicksort(greater))
    }
}

// Merge sort — divide and conquer
let split = (arr) => {
    let mid = arr.len() / 2
    (arr.take(mid), arr.drop(mid))
}

let merge : (Array<i32>, Array<i32>) -> Array<i32> = (a, b) => match (a, b) {
    ([], bs)         => bs
    (as_, [])        => as_
    (a:as_, b:bs)    => if a <= b {
        [a].concat(merge(as_, b:bs))
    } else {
        [b].concat(merge(a:as_, bs))
    }
}

let mergesort : (Array<i32>) -> Array<i32> = (arr) => match arr {
    []      => []
    [x]     => [x]
    arr     => {
        let (left, right) = split(arr)
        merge(mergesort(left), mergesort(right))
    }
}

let main : () -> Effect<()> = do {
    let data = [38, 27, 43, 3, 9, 82, 10, 55, 12, 1]
    println(`Original:  ${data}`)
    println(`Quicksort: ${quicksort(data)}`)
    println(`Mergesort: ${mergesort(data)}`)
}
