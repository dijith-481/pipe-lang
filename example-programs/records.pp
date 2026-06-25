// Records and field access

type Person = {
    name : str
    age  : i32
    city : str
}

let alice : Person = { name: `Alice`, age: 30, city: `Wonderland` }

let bob : Person = { name: `Bob`, age: 25, city: `Builderland` }

// Field access with dot operator
let get_name = (p) => p.name

let is_adult = (p) => p.age >= 18

// Record update (functional style — creates new record)
let with_age = (p: Person, new_age) => { name: p.name, age: new_age, city: p.city }

// Higher-order with records
let adults = (people) => people.filter(is_adult)

let names = (people) => people.map(get_name)

let main = () => {
    println(`Name: ${get_name(alice)}`)
    println(`Is adult: ${is_adult(alice)}`)

    let older = with_age(alice, 31)
    println(`Updated age: ${older.age}`)

    let people = [alice, bob]
    println(`Adults: ${adults(people)}`)
    println(`Names: ${names(people)}`)
}
