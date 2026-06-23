let repeat : (str, i32) -> str = (s, n) => match n {
    0 => ``
    n => `${s}${repeat(s, n - 1)}`
}
let row = (width) => `${repeat(` `, width - 2)}`
let main = () => println(row(10))
