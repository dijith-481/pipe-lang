let foo = (x) => `${x}`
let bar = (x) => `${foo(x)}`
let main = () => println(bar(42))
