// Tiny REPL — interactive expression evaluator
//
// Demonstrates: IO effects (read_line/println), recursion at module level,
// string processing, Option/Result, pattern matching, closures
//
// In v0.1, inputs are simulated since stdin effects are evaluated
// immediately. A real REPL would call read_line() in a loop.

// Parse a number from a string
let parse_num = (s) => {
    let trimmed = s.trim()
    match trimmed.parse_i32() {
        Ok(n) => Some(to_f64(n))
        Err(_) => None
    }
}

// Simple expression evaluation: parse "a + b" format
let eval_line = (line) => {
    let trimmed = line.trim()
    match trimmed {
        ``       => Ok(0.0)
        `exit`   => Err(`bye`)
        `quit`   => Err(`bye`)
        line => {
            let parts = trimmed.split(`+`)
            match parts.len() {
                2 => {
                    let l = match parts[0].trim().parse_i32() { Ok(n) => to_f64(n) _ => 0.0 }
                    let r = match parts[1].trim().parse_i32() { Ok(n) => to_f64(n) _ => 0.0 }
                    Ok(l + r)
                }
                _ => {
                    match trimmed.parse_i32() {
                        Ok(n) => Ok(to_f64(n))
                        Err(_) => { println(`  Unknown: ${trimmed}`); Ok(0.0) }
                    }
                }
            }
        }
    }
}

// Process one line
let process_line = (line) => {
    match eval_line(line) {
        Ok(v) => { println(`  => ${to_str(v)}`); Ok(()) }
        Err(m) => { println(`  ${m}`); Err(m) }
    }
}

// Helper: get nth element using fold
let nth = (arr, n) =>
    arr.fold({ idx: 0, result: `` }, (acc, elem) =>
        if acc.idx == n { { idx: acc.idx + 1, result: elem } }
        else { idx: acc.idx + 1, result: acc.result }
    ).result

// REPL loop (top-level recursion)
let repl_loop = (count) => {
    println(`[${to_str(count)}] > `)
    let inputs = [`42`, `3 + 4`, `10 + 20`, `exit`]
    match count < inputs.len() {
        true => {
            let input = nth(inputs, count)
            println(input)
            match process_line(input) {
                Ok(_) => repl_loop(count + 1)
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
    repl_loop(0)
}
