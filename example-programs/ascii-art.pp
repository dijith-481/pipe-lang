// ASCII art generator — demonstrates string building via template strings

// Recursive string repetition
let repeat : (str, i32) -> str = (s, n) => match n {
    0 => ``
    n => `${s}${repeat(s, n - 1)}`
}

let border = (w) => `+${repeat(`-`, w)}+`

let row = (content, width) => {
    `| ${content}${repeat(` `, width - 2)} |`
}

let box = (text, width) =>
    `${border(width)}\n${row(text, width)}\n${border(width)}`

let main = () => {
    println(box(`Hello!`, 20))
    println(``)
    println(box(`pipe-lang v0.1.0`, 30))
    println(``)

    // Draw a simple tree
    let tree = [
        `    *    `,
        `   ***   `,
        `  *****  `,
        ` ******* `,
        `    |    `
    ]
    tree.map((line) => println(line))
}
