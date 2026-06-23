let main = () => println(row(`hello`, 10))

let repeat : (str, i32) -> str = (s, n) => match n {
    0 => ``
    n => `${s}${repeat(s, n - 1)}`
}
let row = (content, width) => `${repeat(` `, width - 2)}`
