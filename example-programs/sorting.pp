// Sorting Algorithms
//
// Demonstrates: quicksort, merge sort, functional decomposition,
// closures, pattern matching with Option

// Quicksort: partition around pivot, recurse on halves
let quicksort : (Array<i32>) -> Array<i32> = (arr) => match arr.len() {
    0usize => []
    1usize => arr
    _ => {
        let pivot = match arr.head() { Some(v) => v _ => 0 }
        let rest = match arr.tail() { Some(t) => t _ => [] }
        let less = rest.filter((x) => x <= pivot)
        let greater = rest.filter((x) => x > pivot)
        quicksort(less).concat([pivot]).concat(quicksort(greater))
    }
}

// Merge: combine two sorted arrays (workaround: if/else instead of match on tuple of .len())
let merge : (Array<i32>, Array<i32>) -> Array<i32> = (a, b) => {
    let alen = a.len()
    let blen = b.len()
    if alen == 0usize { b }
    else if blen == 0usize { a }
    else {
        let a_head = match a.head() { Some(v) => v _ => 0 }
        let b_head = match b.head() { Some(v) => v _ => 0 }
        if a_head <= b_head {
            let a_rest = match a.tail() { Some(t) => t _ => [] }
            [a_head].concat(merge(a_rest, b))
        } else {
            let b_rest = match b.tail() { Some(t) => t _ => [] }
            [b_head].concat(merge(a, b_rest))
        }
    }
}

// Merge sort: divide and conquer
let merge_sort : (Array<i32>) -> Array<i32> = (arr) => match arr.len() {
    0usize => []
    1usize => arr
    _ => {
        let mid = arr.len() / 2usize
        let left = arr.take(mid)
        let right = arr.drop(mid)
        merge(merge_sort(left), merge_sort(right))
    }
}

let main = () => {
    let data = [38, 27, 43, 3, 9, 82, 10, 55, 12, 1]
    println(`Original:  ${data}`)
    println(`Quicksort: ${quicksort(data)}`)
    println(`Mergesort: ${merge_sort(data)}`)
}
