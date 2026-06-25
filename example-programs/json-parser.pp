// JSON-like Data Demo
//
// Demonstrates: record types, nested data, method chaining,
// string processing, closures

// Get entry value
let get_value = (entry) => entry.value

// Get entry key
let get_key = (entry) => entry.key

// Format entry as string
let entry_to_str = (e) => `${e.key}: ${e.value}`

// -- Main --
let main = () => {
    println(`=== JSON-like Data Demo ===`)
    println(``)

    // Create some entries as records
    let entries = [
        { key: `name`, value: `Alice` },
        { key: `age`, value: `30` },
        { key: `city`, value: `New York` },
        { key: `country`, value: `USA` }
    ]

    println(`All entries:`)
    entries.map((e) => println(`  ${entry_to_str(e)}`))

    println(``)
    println(`Filtered (name):`)
    let name_entries = entries.filter((e) => e.key == `name`)
    name_entries.map((e) => println(`  ${entry_to_str(e)}`))

    println(``)
    println(`Done.`)
}
