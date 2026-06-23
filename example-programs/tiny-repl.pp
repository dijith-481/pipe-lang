// Tiny REPL — interactive expression evaluator
//
// Demonstrates: IO effects (read_line/println), recursion at module level,
// string processing, Option/Result, pattern matching, closures
//
// In v0.1, inputs are simulated since stdin effects are evaluated
// immediately. A real REPL would call read_line() in a loop.

// Supported operations
type Expr =
  | Num(f64)
  | Add(Expr, Expr)
  | Sub(Expr, Expr)
  | Mul(Expr, Expr)
  | Div(Expr, Expr)

// Parse a number from a string
let parseNum = (s) => {
    let trimmed = s.trim()
    match trimmed.parse_i32() {
        Ok(n) => Some(to_f64(n))
        Err(_) => match trimmed {
            // Try parsing as float via to_f64
            s => Some(0.0)  // simplified: always 0 for v0.1
        }
    }
}

// Simple expression evaluation (no parser — build expressions directly)
let eval = (expr) => match expr {
    Num(v)       => Ok(v)
    Add(Num(l), Num(r)) => Ok(l + r)
    Sub(Num(l), Num(r)) => Ok(l - r)
    Mul(Num(l), Num(r)) => Ok(l * r)
    Div(Num(l), Num(r)) =>
        match r == 0.0 {
            true  => Err(`division by zero`)
            false => Ok(l / r)
        }
    Add(l, r) => eval(l).flat_map((lv) => eval(r).flat_map((rv) => Ok(lv + rv)))
    Sub(l, r) => eval(l).flat_map((lv) => eval(r).flat_map((rv) => Ok(lv - rv)))
    Mul(l, r) => eval(l).flat_map((lv) => eval(r).flat_map((rv) => Ok(lv * rv)))
    Div(l, r) => eval(l).flat_map((lv) => eval(r).flat_map((rv) =>
        match rv == 0.0 {
            true  => Err(`division by zero`)
            false => Ok(lv / rv)
        }
    ))
}

let exprToStr = (e) => match e {
    Num(v)       => to_str(v)
    Add(l, r)    => `(${exprToStr(l)} + ${exprToStr(r)})`
    Sub(l, r)    => `(${exprToStr(l)} - ${exprToStr(r)})`
    Mul(l, r)    => `(${exprToStr(l)} * ${exprToStr(r)})`
    Div(l, r)    => `(${exprToStr(l)} / ${exprToStr(r)})`
}

// REPL: process one expression
let processLine = (line) => {
    let trimmed = line.trim()
    match trimmed {
        ``       => Ok(())    // skip empty
        `exit`   => Err(`bye`)  // exit signal
        `quit`   => Err(`bye`)
        line => {
            // For v0.1, parse input as a simple arithmetic string
            // and construct the expression
            let parts = trimmed.split(`+`)
            match parts.len() {
                2 => {
                    let l = match parts[0].trim().parse_i32() { Ok(n) => to_f64(n) Err(_) => 0.0 }
                    let r = match parts[1].trim().parse_i32() { Ok(n) => to_f64(n) Err(_) => 0.0 }
                    let expr = Add(Num(l), Num(r))
                    match eval(expr) {
                        Ok(v) => { println(`  => ${to_str(v)}`); Ok(()) }
                        Err(m) => { println(`  Error: ${m}`); Ok(()) }
                    }
                }
                _ => {
                    // Try as single number
                    match trimmed.parse_i32() {
                        Ok(n) => { println(`  => ${to_str(n)}`); Ok(()) }
                        Err(_) => { println(`  Unknown expression: ${trimmed}`); Ok(()) }
                    }
                }
            }
        }
    }
}

// REPL loop (top-level recursion)
let replLoop = (count) => {
    println(`[${to_str(count)}] > `)  // prompt
    // Simulated inputs for v0.1
    let inputs = [`42`, `3 + 4`, `10 + 20`, `exit`]
    match count < inputs.len() {
        true => {
            let input = inputs[count]
            println(input)  // echo input
            match processLine(input) {
                Ok(_) => replLoop(count + 1)
                Err(_) => println(`bye`)
            }
        }
        false => println(`bye`)
    }
}

let main = () => {
    println(`=== Tiny REPL ===`)
    println(`Simple arithmetic expression evaluator`)
    println(`Enter expressions like: 3 + 4`)
    println(`Type 'exit' to quit`)
    println(``)
    replLoop(0)
}
