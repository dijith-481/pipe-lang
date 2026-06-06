// Pattern matching — exhaustive matching on sum types

type Shape =
  | Circle(f64)
  | Rectangle(f64, f64)
  | Triangle(f64, f64, f64)

let area : (Shape) -> f64 = (shape) => match shape {
    Circle(r)          => 3.14159 * r * r
    Rectangle(w, h)    => w * h
    Triangle(a, b, c)  => {
        // Heron's formula
        let s = (a + b + c) / 2.0
        (s * (s - a) * (s - b) * (s - c)).sqrt()
    }
}

let describe = (shape) => match shape {
    Circle(r)          => `Circle with radius ${r}`
    Rectangle(w, h)    => `Rectangle ${w}x${h}`
    Triangle(a, b, c)  => `Triangle with sides ${a}, ${b}, ${c}`
}

// Nested pattern matching
type Message =
  | Login(str, str)
  | Logout
  | Chat(str, str)
  | Ping

let handleMessage = (msg) => match msg {
    Login(user, _)    => `${user} logged in`
    Logout            => `user logged out`
    Chat(user, text)  => `${user}: ${text}`
    Ping              => `ping received`
}

let main : () -> Effect<()> = do {
    let shapes = [Circle(5.0), Rectangle(4.0, 6.0), Triangle(3.0, 4.0, 5.0)]

    shapes.map((s) => {
        println(`${describe(s)} has area ${area(s)}`)
    })

    println(``)

    let messages = [Login(`alice`, `secret`), Chat(`bob`, `hello`), Ping, Logout]
    messages.map((m) => println(handleMessage(m)))
}
