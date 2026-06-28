let main = () => {
    // Test 1: filter with closure that captures a variable
    let data = [1usize, 2usize, 3usize, 4usize, 5usize]
    let threshold = 3usize
    let filtered = data.filter((x) => x > threshold)
    println(`Filtered > 3: ${to_str(filtered.len())}`)

    // Test 2: fold with closure that captures a variable
    let prefix = 100usize
    let sum = data.fold(0usize, (acc, r) => acc + r + prefix)
    println(`Sum with prefix: ${to_str(sum)}`)

    // Test 3: fold with concat and a capturing closure
    let suffix = 99usize
    let result = data.fold([], (acc, r) => acc.concat([r, suffix]))
    println(`Concat with suffix: ${to_str(result.len())}`)
}
