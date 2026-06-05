// Hello World — the simplest pipe-lang program
use stdlib.io

let main : () -> Effect<Unit>
let main = do {
    IO.println("Hello, World!")
}
