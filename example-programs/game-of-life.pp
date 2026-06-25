// Game of Life Demo
//
// Demonstrates: array manipulation, fold, closures,
// string building with templates

// Count live neighbors (simplified: just count offsets)
let count_neighbors = (row, col, rows, cols) => {
    let offsets = [-1, 0, 1]
    offsets.fold(0, (count, dr) =>
        offsets.fold(count, (inner, dc) => {
            let nr = row + dr
            let nc = col + dc
            match nr >= 0 && nr < rows && nc >= 0 && nc < cols && !(dr == 0 && dc == 0) {
                true => inner + 1
                false => inner
            }
        })
    )
}

// Convert cell to string
let cell_to_str = (alive) => match alive {
    true => `#`
    false => `.`
}

// -- Main --
let main = () => {
    println(`=== Game of Life ===`)
    println(``)
    println(`Conway's Game of Life is a cellular automaton where:`)
    println(`- A live cell with 2 or 3 neighbors survives`)
    println(`- A dead cell with exactly 3 neighbors becomes alive`)
    println(`- All other cells die or stay dead`)
    println(``)
    println(`Initial glider pattern:`)
    println(`. # . . . . . .`)
    println(`. . # . . . . .`)
    println(`# # # . . . . .`)
    println(`. . . . . . . .`)
    println(`. . . . . . . .`)
    println(`. . . . . . . .`)
    println(`. . . . . . . .`)
    println(`. . . . . . . .`)
    println(``)
    println(`Done.`)
}
