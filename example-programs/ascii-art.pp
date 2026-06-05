// ASCII art generator — demonstrates string building
use stdlib.io

let repeat : (str, i32) -> str
let repeat = (s, n) => match n {
    0 => ""
    n => s ++ repeat(s, n - 1)
}

let border : (i32) -> str
let border = (w) => "+" ++ repeat("-", w) ++ "+"

let row : (str, i32) -> str
let row = (content, width) => {
    let padding = width - content.len() - 2
    "| " ++ content ++ repeat(" ", padding) ++ " |"
}

let box : (str, i32) -> str
let box = (text, width) => {
    border(width) ++ "\n" ++
    row(text, width) ++ "\n" ++
    border(width)
}

let main : () -> Effect<Unit>
let main = do {
    IO.println(box("Hello!", 20))
    IO.println("")
    IO.println(box("pipe-lang v0.1.0", 30))
    IO.println("")

    // Draw a simple tree
    let tree = [
        "    *    ",
        "   ***   ",
        "  *****  ",
        " ******* ",
        "    |    "
    ]
    tree.map((line) => IO.println(line))
}
