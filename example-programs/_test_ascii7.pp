let repeat : (str, i32) -> str = (s, n) => match n {
    0 => ``
    n => `${s}${repeat(s, n - 1)}`
}
let border = (w) => `+${repeat(`-`, w)}+`
let row = (content, width) => {
    `| ${content}${repeat(` `, width - 2)} |`
}
let box = (text, width) => `${border(width)} ${row(text, width)}`
let main = () => println(box(`Hello!`, 20))
