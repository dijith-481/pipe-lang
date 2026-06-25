// Expression Evaluator — symbolic arithmetic expression evaluation
//
// Demonstrates: ADTs, recursive pattern matching, Option/Result chaining,
// closures, higher-order functions, pure computation

type Expr =
  | Num(f64)
  | Add(Expr, Expr)
  | Sub(Expr, Expr)
  | Mul(Expr, Expr)
  | Div(Expr, Expr)
  | Neg(Expr)

// -- Evaluation --
let eval = (expr) => match expr {
    Num(v)           => Ok(v)
    Neg(val)         => eval(val).flatMap((v) => Ok(-v))
    Add(left, right) => eval(left).flatMap((l) => eval(right).flatMap((r) => Ok(l + r)))
    Sub(left, right) => eval(left).flatMap((l) => eval(right).flatMap((r) => Ok(l - r)))
    Mul(left, right) => eval(left).flatMap((l) => eval(right).flatMap((r) => Ok(l * r)))
    Div(left, right) => eval(left).flatMap((l) => eval(right).flatMap((r) =>
        match r == 0.0 {
            true  => Err(`division by zero`)
            false => Ok(l / r)
        }
    ))
}

// -- Display --
let expr_to_str = (expr) => match expr {
    Num(v)           => to_str(v)
    Add(left, right) => `(${expr_to_str(left)} + ${expr_to_str(right)})`
    Sub(left, right) => `(${expr_to_str(left)} - ${expr_to_str(right)})`
    Mul(left, right) => `(${expr_to_str(left)} * ${expr_to_str(right)})`
    Div(left, right) => `(${expr_to_str(left)} / ${expr_to_str(right)})`
    Neg(val)         => `(-${expr_to_str(val)})`
}

let format_result = (expr, result) => match result {
    Ok(v)    => `${expr_to_str(expr)} = ${to_str(v)}`
    Err(msg) => `${expr_to_str(expr)} => Error: ${msg}`
}

// -- Tests --
let run_test = (expr) => {
    let result = eval(expr)
    println(format_result(expr, result))
}

let main = () => {
    println(`=== Expression Evaluator ===`)
    println(``)

    // Simple arithmetic: 2 + 3 * 4
    run_test(Add(Num(2.0), Mul(Num(3.0), Num(4.0))))

    // Parenthesized grouping: (2 + 3) * 4
    run_test(Mul(Add(Num(2.0), Num(3.0)), Num(4.0)))

    // Division: 10 / 3
    run_test(Div(Num(10.0), Num(3.0)))

    // Negation: -5 + 3
    run_test(Add(Neg(Num(5.0)), Num(3.0)))

    // Chained addition: 1 + 2 + 3 + 4
    run_test(Add(Num(1.0), Add(Num(2.0), Add(Num(3.0), Num(4.0)))))

    // Complex: (1 + 2) * (3 + 4) / (5 - 1)
    run_test(Div(
        Mul(Add(Num(1.0), Num(2.0)), Add(Num(3.0), Num(4.0))),
        Sub(Num(5.0), Num(1.0))
    ))

    // Error case: division by zero
    run_test(Div(Num(1.0), Num(0.0)))
}
