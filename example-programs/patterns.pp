// Pattern matching — exhaustive matching on sum types
use stdlib.io

type Shape =
  | Circle(f64)
  | Rectangle(f64, f64)
  | Triangle(f64, f64, f64)

let area : (Shape) -> f64
let area = (shape) => match shape {
    Circle(r)          => 3.14159 * r * r
    Rectangle(w, h)    => w * h
    Triangle(a, b, c)  => {
        // Heron's formula
        let s = (a + b + c) / 2.0
        (s * (s - a) * (s - b) * (s - c)).sqrt()
    }
}

let describe : (Shape) -> str
let describe = (shape) => match shape {
    Circle(r)          => "Circle with radius " ++ r.toString()
    Rectangle(w, h)    => "Rectangle " ++ w.toString() ++ "x" ++ h.toString()
    Triangle(a, b, c)  => "Triangle with sides " ++ a.toString() ++ ", " ++ b.toString() ++ ", " ++ c.toString()
}

// Nested pattern matching
type Message =
  | Login(str, str)
  | Logout
  | Chat(str, str)
  | Ping

let handleMessage : (Message) -> str
let handleMessage = (msg) => match msg {
    Login(user, _)    => user ++ " logged in"
    Logout            => "user logged out"
    Chat(user, text)  => user ++ ": " ++ text
    Ping              => "ping received"
}

let main : () -> Effect<Unit>
let main = do {
    let shapes = [Circle(5.0), Rectangle(4.0, 6.0), Triangle(3.0, 4.0, 5.0)]

    shapes.map((s) => {
        IO.println(describe(s) ++ " has area " ++ area(s).toString())
    })

    IO.println("")

    let messages = [Login("alice", "secret"), Chat("bob", "hello"), Ping, Logout]
    messages.map((m) => IO.println(handleMessage(m)))
}
