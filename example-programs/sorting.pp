// Quicksort — classic functional algorithm

let quicksort = (arr) => match arr {
    []         => []
    [pivot]    => [pivot]
    arr        => {
        let pivot = arr[0]
        let rest = arr.drop(1)
        let less = rest.filter((x) => x <= pivot)
        let greater = rest.filter((x) => x > pivot)
        quicksort(less) ++ [pivot] ++ quicksort(greater)
    }
}

// Merge sort — divide and conquer
let split = (arr) => {
    let mid = arr.len() / 2
    (arr.take(mid), arr.drop(mid))
}

let merge = (a, b) => match (a, b) {
    ([], bs)         => bs
    (as_, [])        => as_
    (a:as_, b:bs)    => if a <= b {
        [a] ++ merge(as_, b:bs)
    } else {
        [b] ++ merge(a:as_, bs)
    }
}

let mergesort = (arr) => match arr {
    []      => []
    [x]     => [x]
    arr     => {
        let (left, right) = split(arr)
        merge(mergesort(left), mergesort(right))
    }
}

let main : () -> Effect<()> = do {
    let data = [38, 27, 43, 3, 9, 82, 10, 55, 12, 1]
    println("Original:  " ++ data.toString())
    println("Quicksort: " ++ quicksort(data).toString())
    println("Mergesort: " ++ mergesort(data).toString())
}
