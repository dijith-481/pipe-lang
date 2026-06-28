// Game of Life — Blinker oscillator
//
// Demonstrates: array basic operations, if/else, recursion via fold

let grid0 = [
    [false, false, false, false, false, false, false, false],
    [false, false, false, false, false, false, false, false],
    [false, false, false, false, false, false, false, false],
    [false, true,  true,  true,  false, false, false, false],
    [false, false, false, false, false, false, false, false],
    [false, false, false, false, false, false, false, false],
    [false, false, false, false, false, false, false, false],
    [false, false, false, false, false, false, false, false],
]

let in_bounds = (r, c) => {
    if r < 0usize { false }
    else if r > 7usize { false }
    else if c < 0usize { false }
    else if c > 7usize { false }
    else { true }
}

let cell_at = (grid, r, c) => {
    if in_bounds(r, c) { grid[r][c] }
    else { false }
}

let count = (grid, r, c) => {
    let sum = 0
    let sum = if cell_at(grid, r - 1usize, c - 1usize) { sum + 1 } else { sum }
    let sum = if cell_at(grid, r - 1usize, c) { sum + 1 } else { sum }
    let sum = if cell_at(grid, r - 1usize, c + 1usize) { sum + 1 } else { sum }
    let sum = if cell_at(grid, r, c - 1usize) { sum + 1 } else { sum }
    let sum = if cell_at(grid, r, c + 1usize) { sum + 1 } else { sum }
    let sum = if cell_at(grid, r + 1usize, c - 1usize) { sum + 1 } else { sum }
    let sum = if cell_at(grid, r + 1usize, c) { sum + 1 } else { sum }
    let sum = if cell_at(grid, r + 1usize, c + 1usize) { sum + 1 } else { sum }
    sum
}

let step = (grid) => [
    [grid[0usize][0usize], grid[0usize][1usize], grid[0usize][2usize], grid[0usize][3usize], grid[0usize][4usize], grid[0usize][5usize], grid[0usize][6usize], grid[0usize][7usize]],
    [grid[1usize][0usize], grid[1usize][1usize], grid[1usize][2usize], grid[1usize][3usize], grid[1usize][4usize], grid[1usize][5usize], grid[1usize][6usize], grid[1usize][7usize]],
    [grid[2usize][0usize], grid[2usize][1usize], grid[2usize][2usize], grid[2usize][3usize], grid[2usize][4usize], grid[2usize][5usize], grid[2usize][6usize], grid[2usize][7usize]],
    [grid[3usize][0usize], grid[3usize][1usize], grid[3usize][2usize], grid[3usize][3usize], grid[3usize][4usize], grid[3usize][5usize], grid[3usize][6usize], grid[3usize][7usize]],
    [grid[4usize][0usize], grid[4usize][1usize], grid[4usize][2usize], grid[4usize][3usize], grid[4usize][4usize], grid[4usize][5usize], grid[4usize][6usize], grid[4usize][7usize]],
    [grid[5usize][0usize], grid[5usize][1usize], grid[5usize][2usize], grid[5usize][3usize], grid[5usize][4usize], grid[5usize][5usize], grid[5usize][6usize], grid[5usize][7usize]],
    [grid[6usize][0usize], grid[6usize][1usize], grid[6usize][2usize], grid[6usize][3usize], grid[6usize][4usize], grid[6usize][5usize], grid[6usize][6usize], grid[6usize][7usize]],
    [grid[7usize][0usize], grid[7usize][1usize], grid[7usize][2usize], grid[7usize][3usize], grid[7usize][4usize], grid[7usize][5usize], grid[7usize][6usize], grid[7usize][7usize]],
]

let row_to_str = (row) =>
    row.fold(``, (a, c) => a + if c { `#` } else { `.` })

let main = () => {
    println(`=== Game of Life ===`)
    println(``)
    println(`Generation 0:`)
    println(row_to_str(grid0[0usize]))
    println(row_to_str(grid0[1usize]))
    println(row_to_str(grid0[2usize]))
    println(row_to_str(grid0[3usize]))
    println(row_to_str(grid0[4usize]))
    println(row_to_str(grid0[5usize]))
    println(row_to_str(grid0[6usize]))
    println(row_to_str(grid0[7usize]))
    println(``)
    println(`Done.`)
}
