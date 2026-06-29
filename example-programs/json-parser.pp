// JSON-like Data Demo
//
// Demonstrates: record types, recursive functions, arrays

let entries = [
    { key: `name`, value: `Alice` },
    { key: `age`, value: `30` },
    { key: `city`, value: `New York` },
    { key: `country`, value: `USA` }
]

let print_all = (i) => match i < entries.len() {
    true => {
        let e = entries[i]
        println(`  ${e.key}: ${e.value}`)
        print_all(i + 1usize)
    }
    false => true
}

let main = () => {
    println(`=== JSON-like Data Demo ===`)
    println(``)
    println(`All entries:`)
    let _ = print_all(0usize)
    println(``)
    println(`Done.`)
}
