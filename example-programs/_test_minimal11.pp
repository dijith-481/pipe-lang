let repeat : (str, i32) -> str = (s, n) => match n {
    0 => ``
    n => `${s}${repeat(s, n - 1)}`
}
let foo = (s, n) => repeat(s, n)
let row = (content, width) => `${foo(` `, width - 2)}`
let main = () => println(row(`hello`, 10))
