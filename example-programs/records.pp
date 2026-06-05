// Records and field access
use stdlib.io

type Person = {
    name : str
    age  : i32
    city : str
}

let alice : Person
let alice = { name: "Alice", age: 30, city: "Wonderland" }

let bob : Person
let bob = { name: "Bob", age: 25, city: "Builderland" }

// Field access with dot operator
let getName : (Person) -> str
let getName = (p) => p.name

let isAdult : (Person) -> bool
let isAdult = (p) => p.age >= 18

// Record update (functional style — creates new record)
let withAge : (Person, i32) -> Person
let withAge = (p, newAge) => { name: p.name, age: newAge, city: p.city }

// Higher-order with records
let adults : (Array<Person>) -> Array<Person>
let adults = (people) => people.filter(isAdult)

let names : (Array<Person>) -> Array<str>
let names = (people) => people.map(getName)

let main : () -> Effect<Unit>
let main = do {
    IO.println("Name: " ++ getName(alice))
    IO.println("Is adult: " ++ isAdult(alice).toString())

    let older = withAge(alice, 31)
    IO.println("Updated age: " ++ older.age.toString())

    let people = [alice, bob]
    IO.println("Adults: " ++ adults(people).toString())
    IO.println("Names: " ++ names(people).toString())
}
