// Expression Evaluator
//
// Demonstrates: ADTs, recursion, pattern matching

type Expr =
  | Num(f64)
  | Add(Expr, Expr)
  | Sub(Expr, Expr)
  | Mul(Expr, Expr)
  | Div(Expr, Expr)
  | Neg(Expr)

// Use Err(e) to preserve the error value, NOT x => x which causes type issues
let eval: (Expr) -> Result<f64, str> = (expr) => match expr {
    Num(v) => Ok(v)
    Neg(val) => { let r = eval(val); match r { Ok(v) => Ok(-v) Err(e) => Err(e) } }
    Add(l, r) => { let lr = eval(l); match lr { Ok(lv) => { let rr = eval(r); match rr { Ok(rv) => Ok(lv + rv) Err(e) => Err(e) } } Err(e) => Err(e) } }
    Sub(l, r) => { let lr = eval(l); match lr { Ok(lv) => { let rr = eval(r); match rr { Ok(rv) => Ok(lv - rv) Err(e) => Err(e) } } Err(e) => Err(e) } }
    Mul(l, r) => { let lr = eval(l); match lr { Ok(lv) => { let rr = eval(r); match rr { Ok(rv) => Ok(lv * rv) Err(e) => Err(e) } } Err(e) => Err(e) } }
    Div(l, r) => { let lr = eval(l); match lr { Ok(lv) => { let rr = eval(r); match rr { Ok(rv) => if rv == 0.0 { Err(`division by zero`) } else { Ok(lv / rv) } Err(e) => Err(e) } } Err(e) => Err(e) } }
}

let expr_to_str: (Expr) -> str = (expr) => match expr { Num(v) => to_str(v) Add(l,r) => `(${expr_to_str(l)} + ${expr_to_str(r)})` Sub(l,r) => `(${expr_to_str(l)} - ${expr_to_str(r)})` Mul(l,r) => `(${expr_to_str(l)} * ${expr_to_str(r)})` Div(l,r) => `(${expr_to_str(l)} / ${expr_to_str(r)})` Neg(val) => `(-${expr_to_str(val)})` }
let run_test = (expr) => { let repr = expr_to_str(expr); match eval(expr) { Ok(v) => println(`${repr} = ${to_str(v)}`) Err(msg) => println(`${repr} => Error: ${msg}`) } }

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
