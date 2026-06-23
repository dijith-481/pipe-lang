let repeat : (str, i32) -> str = (s, n) => match n {
    0 => ``
    n => `${s}${repeat(s, n - 1)}`
}
let border = (w) => `+${repeat(`-`, w)}+`
let row = (content, width) => {
    `| ${content}${repeat(` `, width - 2)} |`
}
let main = () => println(border(20))
