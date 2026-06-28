// Expression Evaluator
//
// Demonstrates: ADTs, flat_map for error propagation

type Expr =
  | Num(f64)
  | Add(Expr, Expr)
  | Sub(Expr, Expr)
  | Mul(Expr, Expr)
  | Div(Expr, Expr)
  | Neg(Expr)

// Use if/else instead of match on Bool (pre-existing JIT bug)
// Use flat_map instead of direct match on Result (avoids TagGet issue in recursive funcs)
let eval: (Expr) -> Result<f64, str> = (expr) => match expr {
    Num(v) => Ok(v)
    Neg(val) => eval(val).flat_map((v) => Ok(-v))
    Add(l, r) => eval(l).flat_map((lv) => eval(r).flat_map((rv) => Ok(lv + rv)))
    Sub(l, r) => eval(l).flat_map((lv) => eval(r).flat_map((rv) => Ok(lv - rv)))
    Mul(l, r) => eval(l).flat_map((lv) => eval(r).flat_map((rv) => Ok(lv * rv)))
    Div(l, r) => eval(l).flat_map((lv) => eval(r).flat_map((rv) => if rv == 0.0 { Err(`division by zero`) } else { Ok(lv / rv) }))
}

let expr_to_str = (expr) => match expr { Num(v) => to_str(v) Add(l,r) => `(${expr_to_str(l)} + ${expr_to_str(r)})` Sub(l,r) => `(${expr_to_str(l)} - ${expr_to_str(r)})` Mul(l,r) => `(${expr_to_str(l)} * ${expr_to_str(r)})` Div(l,r) => `(${expr_to_str(l)} / ${expr_to_str(r)})` Neg(val) => `(-${expr_to_str(val)})` }
let format_result = (expr, result) => match result { Ok(v) => `${expr_to_str(expr)} = ${to_str(v)}` Err(msg) => `${expr_to_str(expr)} => Error: ${msg}` }
let run_test = (expr) => { let result = eval(expr); println(format_result(expr, result)) }

let main = () => {
    println(`=== Expression Evaluator ===`)
    println(``)
    run_test(Add(Num(2.0), Mul(Num(3.0), Num(4.0))))
    run_test(Mul(Add(Num(2.0), Num(3.0)), Num(4.0)))
    run_test(Div(Num(10.0), Num(3.0)))
    run_test(Add(Neg(Num(5.0)), Num(3.0)))
    run_test(Add(Num(1.0), Add(Num(2.0), Add(Num(3.0), Num(4.0)))))
    run_test(Div(
        Mul(Add(Num(1.0), Num(2.0)), Add(Num(3.0), Num(4.0))),
        Sub(Num(5.0), Num(1.0))
    ))
    run_test(Div(Num(1.0), Num(0.0)))
}
