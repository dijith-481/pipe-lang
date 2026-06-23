// Expression Evaluator — symbolic arithmetic expression evaluation
//
// Demonstrates: ADTs, recursive pattern matching, Option/Result chaining,
// closures, higher-order functions, pure computation
//
// Expressions are built directly (not parsed from strings) to focus on
// ADT manipulation and evaluation.

type Expr =
  | Num(f64)
  | Add(Expr, Expr)
  | Sub(Expr, Expr)
  | Mul(Expr, Expr)
  | Div(Expr, Expr)
  | Neg(Expr)
  | Sqrt(Expr)

// -- Evaluation --
let eval = (expr) => match expr {
    Num(v)           => Ok(v)
    Add(Num(l), Num(r)) => Ok(l + r)
    Sub(Num(l), Num(r)) => Ok(l - r)
    Mul(Num(l), Num(r)) => Ok(l * r)
    Div(Num(l), Num(r)) =>
        match r == 0.0 {
            true  => Err(`division by zero`)
            false => Ok(l / r)
        }
    Neg(Num(v))      => Ok(-v)
    Sqrt(Num(v))     =>
        match v < 0.0 {
            true  => Err(`sqrt of negative number`)
            false => Ok(v.sqrt())
        }
    // Recursive cases
    Add(left, right) => eval(left).flat_map((l) => eval(right).flat_map((r) => Ok(l + r)))
    Sub(left, right) => eval(left).flat_map((l) => eval(right).flat_map((r) => Ok(l - r)))
    Mul(left, right) => eval(left).flat_map((l) => eval(right).flat_map((r) => Ok(l * r)))
    Div(left, right) => eval(left).flat_map((l) => eval(right).flat_map((r) =>
        match r == 0.0 {
            true  => Err(`division by zero`)
            false => Ok(l / r)
        }
    ))
    Neg(val)         => eval(val).flat_map((v) => Ok(-v))
    Sqrt(val)        => eval(val).flat_map((v) =>
        match v < 0.0 {
            true  => Err(`sqrt of negative number`)
            false => Ok(v.sqrt())
        }
    )
}

// -- Display --
let exprToString = (expr) => match expr {
    Num(v)           => to_str(v)
    Add(left, right) => `(${exprToString(left)} + ${exprToString(right)})`
    Sub(left, right) => `(${exprToString(left)} - ${exprToString(right)})`
    Mul(left, right) => `(${exprToString(left)} * ${exprToString(right)})`
    Div(left, right) => `(${exprToString(left)} / ${exprToString(right)})`
    Neg(val)         => `(-${exprToString(val)})`
    Sqrt(val)        => `sqrt(${exprToString(val)})`
}

let formatResult = (expr, result) => match result {
    Ok(v)    => `${exprToString(expr)} = ${to_str(v)}`
    Err(msg) => `${exprToString(expr)} => Error: ${msg}`
}

// -- Tests --
let runTest = (expr) => {
    let result = eval(expr)
    println(formatResult(expr, result))
}

let main = () => {
    println(`=== Expression Evaluator ===`)
    println(``)

    // Simple arithmetic: 2 + 3 * 4 = 14
    runTest(Add(Num(2.0), Mul(Num(3.0), Num(4.0))))

    // Parenthesized grouping: (2 + 3) * 4 = 20
    runTest(Mul(Add(Num(2.0), Num(3.0)), Num(4.0)))

    // Division: 10 / 3 ≈ 3.333...
    runTest(Div(Num(10.0), Num(3.0)))

    // Negation: -5 + 3 = -2
    runTest(Add(Neg(Num(5.0)), Num(3.0)))

    // Square root: sqrt(16) = 4
    runTest(Sqrt(Num(16.0)))

    // Chained addition: 1 + 2 + 3 + 4 = 10
    runTest(Add(Num(1.0), Add(Num(2.0), Add(Num(3.0), Num(4.0)))))

    // Complex: (1 + 2) * (3 + 4) / (5 - 1) = 21 / 4 = 5.25
    runTest(Div(
        Mul(Add(Num(1.0), Num(2.0)), Add(Num(3.0), Num(4.0))),
        Sub(Num(5.0), Num(1.0))
    ))

    // Error case: division by zero
    runTest(Div(Num(1.0), Num(0.0)))

    // Error case: sqrt of negative
    runTest(Sqrt(Num(-1.0)))

    // Nested: sqrt(-sqrt(16) + 20) = sqrt(16) = 4
    runTest(Sqrt(Add(Neg(Sqrt(Num(16.0))), Num(20.0))))
}
