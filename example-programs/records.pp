// Records and field access

type Person = {
    name : str
    age  : i32
    city : str
}

let alice : Person = { name: "Alice", age: 30, city: "Wonderland" }

let bob : Person = { name: "Bob", age: 25, city: "Builderland" }

// Field access with dot operator
let getName = (p) => p.name

let isAdult = (p) => p.age >= 18

// Record update (functional style — creates new record)
let withAge = (p, newAge) => { name: p.name, age: newAge, city: p.city }

// Higher-order with records
let adults = (people) => people.filter(isAdult)

let names = (people) => people.map(getName)

let main : () -> Effect<()> = do {
    println("Name: " ++ getName(alice))
    println("Is adult: " ++ isAdult(alice).toString())

    let older = withAge(alice, 31)
    println("Updated age: " ++ older.age.toString())

    let people = [alice, bob]
    println("Adults: " ++ adults(people).toString())
    println("Names: " ++ names(people).toString())
}
