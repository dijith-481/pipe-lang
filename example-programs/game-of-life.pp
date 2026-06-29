// Game of Life — Conway's Game of Life simulation
//
// Demonstrates: multi-dimensional array access, manual iteration via unfold

let rows = 8usize
let cols = 8usize

// Glider pattern (moves diagonally)
let initial = [
    [false, false, false, false, false, false, false, false],
    [false, false, true,  false, false, false, false, false],
    [false, false, false, true,  false, false, false, false],
    [false, true,  true,  true,  false, false, false, false],
    [false, false, false, false, false, false, false, false],
    [false, false, false, false, false, false, false, false],
    [false, false, false, false, false, false, false, false],
    [false, false, false, false, false, false, false, false],
]

let in_bounds = (r, c) =>
    if r < 0usize { false }
    else if r >= rows { false }
    else if c < 0usize { false }
    else if c >= cols { false }
    else { true }

let cell_at = (grid, r, c) =>
    if in_bounds(r, c) { grid[r][c] } else { false }

let count = (grid, r, c) => {
    let n = 0

    let n = if cell_at(grid, r - 1usize, c - 1usize) { n + 1 } else { n }
    let n = if cell_at(grid, r - 1usize, c) { n + 1 } else { n }
    let n = if cell_at(grid, r - 1usize, c + 1usize) { n + 1 } else { n }
    let n = if cell_at(grid, r, c - 1usize) { n + 1 } else { n }
    let n = if cell_at(grid, r, c + 1usize) { n + 1 } else { n }
    let n = if cell_at(grid, r + 1usize, c - 1usize) { n + 1 } else { n }
    let n = if cell_at(grid, r + 1usize, c) { n + 1 } else { n }
    let n = if cell_at(grid, r + 1usize, c + 1usize) { n + 1 } else { n }

    n
}

let next = (grid) => [
    [count(grid, 0usize, 0usize),      count(grid, 0usize, 1usize),      count(grid, 0usize, 2usize),      count(grid, 0usize, 3usize),      count(grid, 0usize, 4usize),      count(grid, 0usize, 5usize),      count(grid, 0usize, 6usize),      count(grid, 0usize, 7usize)],
    [count(grid, 1usize, 0usize),      count(grid, 1usize, 1usize),      count(grid, 1usize, 2usize),      count(grid, 1usize, 3usize),      count(grid, 1usize, 4usize),      count(grid, 1usize, 5usize),      count(grid, 1usize, 6usize),      count(grid, 1usize, 7usize)],
    [count(grid, 2usize, 0usize),      count(grid, 2usize, 1usize),      count(grid, 2usize, 2usize),      count(grid, 2usize, 3usize),      count(grid, 2usize, 4usize),      count(grid, 2usize, 5usize),      count(grid, 2usize, 6usize),      count(grid, 2usize, 7usize)],
    [count(grid, 3usize, 0usize),      count(grid, 3usize, 1usize),      count(grid, 3usize, 2usize),      count(grid, 3usize, 3usize),      count(grid, 3usize, 4usize),      count(grid, 3usize, 5usize),      count(grid, 3usize, 6usize),      count(grid, 3usize, 7usize)],
    [count(grid, 4usize, 0usize),      count(grid, 4usize, 1usize),      count(grid, 4usize, 2usize),      count(grid, 4usize, 3usize),      count(grid, 4usize, 4usize),      count(grid, 4usize, 5usize),      count(grid, 4usize, 6usize),      count(grid, 4usize, 7usize)],
    [count(grid, 5usize, 0usize),      count(grid, 5usize, 1usize),      count(grid, 5usize, 2usize),      count(grid, 5usize, 3usize),      count(grid, 5usize, 4usize),      count(grid, 5usize, 5usize),      count(grid, 5usize, 6usize),      count(grid, 5usize, 7usize)],
    [count(grid, 6usize, 0usize),      count(grid, 6usize, 1usize),      count(grid, 6usize, 2usize),      count(grid, 6usize, 3usize),      count(grid, 6usize, 4usize),      count(grid, 6usize, 5usize),      count(grid, 6usize, 6usize),      count(grid, 6usize, 7usize)],
    [count(grid, 7usize, 0usize),      count(grid, 7usize, 1usize),      count(grid, 7usize, 2usize),      count(grid, 7usize, 3usize),      count(grid, 7usize, 4usize),      count(grid, 7usize, 5usize),      count(grid, 7usize, 6usize),      count(grid, 7usize, 7usize)],
]

let alive = (cur, n) =>
    if cur && (n == 2 || n == 3) { true }
    else if !cur && n == 3 { true }
    else { false }

let step = (grid) => [
    [alive(grid[0usize][0usize], count(grid, 0usize, 0usize)),      alive(grid[0usize][1usize], count(grid, 0usize, 1usize)),      alive(grid[0usize][2usize], count(grid, 0usize, 2usize)),      alive(grid[0usize][3usize], count(grid, 0usize, 3usize)),      alive(grid[0usize][4usize], count(grid, 0usize, 4usize)),      alive(grid[0usize][5usize], count(grid, 0usize, 5usize)),      alive(grid[0usize][6usize], count(grid, 0usize, 6usize)),      alive(grid[0usize][7usize], count(grid, 0usize, 7usize))],
    [alive(grid[1usize][0usize], count(grid, 1usize, 0usize)),      alive(grid[1usize][1usize], count(grid, 1usize, 1usize)),      alive(grid[1usize][2usize], count(grid, 1usize, 2usize)),      alive(grid[1usize][3usize], count(grid, 1usize, 3usize)),      alive(grid[1usize][4usize], count(grid, 1usize, 4usize)),      alive(grid[1usize][5usize], count(grid, 1usize, 5usize)),      alive(grid[1usize][6usize], count(grid, 1usize, 6usize)),      alive(grid[1usize][7usize], count(grid, 1usize, 7usize))],
    [alive(grid[2usize][0usize], count(grid, 2usize, 0usize)),      alive(grid[2usize][1usize], count(grid, 2usize, 1usize)),      alive(grid[2usize][2usize], count(grid, 2usize, 2usize)),      alive(grid[2usize][3usize], count(grid, 2usize, 3usize)),      alive(grid[2usize][4usize], count(grid, 2usize, 4usize)),      alive(grid[2usize][5usize], count(grid, 2usize, 5usize)),      alive(grid[2usize][6usize], count(grid, 2usize, 6usize)),      alive(grid[2usize][7usize], count(grid, 2usize, 7usize))],
    [alive(grid[3usize][0usize], count(grid, 3usize, 0usize)),      alive(grid[3usize][1usize], count(grid, 3usize, 1usize)),      alive(grid[3usize][2usize], count(grid, 3usize, 2usize)),      alive(grid[3usize][3usize], count(grid, 3usize, 3usize)),      alive(grid[3usize][4usize], count(grid, 3usize, 4usize)),      alive(grid[3usize][5usize], count(grid, 3usize, 5usize)),      alive(grid[3usize][6usize], count(grid, 3usize, 6usize)),      alive(grid[3usize][7usize], count(grid, 3usize, 7usize))],
    [alive(grid[4usize][0usize], count(grid, 4usize, 0usize)),      alive(grid[4usize][1usize], count(grid, 4usize, 1usize)),      alive(grid[4usize][2usize], count(grid, 4usize, 2usize)),      alive(grid[4usize][3usize], count(grid, 4usize, 3usize)),      alive(grid[4usize][4usize], count(grid, 4usize, 4usize)),      alive(grid[4usize][5usize], count(grid, 4usize, 5usize)),      alive(grid[4usize][6usize], count(grid, 4usize, 6usize)),      alive(grid[4usize][7usize], count(grid, 4usize, 7usize))],
    [alive(grid[5usize][0usize], count(grid, 5usize, 0usize)),      alive(grid[5usize][1usize], count(grid, 5usize, 1usize)),      alive(grid[5usize][2usize], count(grid, 5usize, 2usize)),      alive(grid[5usize][3usize], count(grid, 5usize, 3usize)),      alive(grid[5usize][4usize], count(grid, 5usize, 4usize)),      alive(grid[5usize][5usize], count(grid, 5usize, 5usize)),      alive(grid[5usize][6usize], count(grid, 5usize, 6usize)),      alive(grid[5usize][7usize], count(grid, 5usize, 7usize))],
    [alive(grid[6usize][0usize], count(grid, 6usize, 0usize)),      alive(grid[6usize][1usize], count(grid, 6usize, 1usize)),      alive(grid[6usize][2usize], count(grid, 6usize, 2usize)),      alive(grid[6usize][3usize], count(grid, 6usize, 3usize)),      alive(grid[6usize][4usize], count(grid, 6usize, 4usize)),      alive(grid[6usize][5usize], count(grid, 6usize, 5usize)),      alive(grid[6usize][6usize], count(grid, 6usize, 6usize)),      alive(grid[6usize][7usize], count(grid, 6usize, 7usize))],
    [alive(grid[7usize][0usize], count(grid, 7usize, 0usize)),      alive(grid[7usize][1usize], count(grid, 7usize, 1usize)),      alive(grid[7usize][2usize], count(grid, 7usize, 2usize)),      alive(grid[7usize][3usize], count(grid, 7usize, 3usize)),      alive(grid[7usize][4usize], count(grid, 7usize, 4usize)),      alive(grid[7usize][5usize], count(grid, 7usize, 5usize)),      alive(grid[7usize][6usize], count(grid, 7usize, 6usize)),      alive(grid[7usize][7usize], count(grid, 7usize, 7usize))],
]

let row_str = (row) =>
    row.fold(``, (a, c) =>
        a + if c { `#` } else { `.` }
    )

let print_gen = (gen, grid) => {
    println(`Generation ${to_str(gen)}:`)
    println(row_str(grid[0usize]))
    println(row_str(grid[1usize]))
    println(row_str(grid[2usize]))
    println(row_str(grid[3usize]))
    println(row_str(grid[4usize]))
    println(row_str(grid[5usize]))
    println(row_str(grid[6usize]))
    println(row_str(grid[7usize]))
}

let any_alive = (grid) =>
    grid[0usize][0usize] || grid[0usize][1usize] || grid[0usize][2usize] || grid[0usize][3usize] ||
    grid[0usize][4usize] || grid[0usize][5usize] || grid[0usize][6usize] || grid[0usize][7usize] ||
    grid[1usize][0usize] || grid[1usize][1usize] || grid[1usize][2usize] || grid[1usize][3usize] ||
    grid[1usize][4usize] || grid[1usize][5usize] || grid[1usize][6usize] || grid[1usize][7usize] ||
    grid[2usize][0usize] || grid[2usize][1usize] || grid[2usize][2usize] || grid[2usize][3usize] ||
    grid[2usize][4usize] || grid[2usize][5usize] || grid[2usize][6usize] || grid[2usize][7usize] ||
    grid[3usize][0usize] || grid[3usize][1usize] || grid[3usize][2usize] || grid[3usize][3usize] ||
    grid[3usize][4usize] || grid[3usize][5usize] || grid[3usize][6usize] || grid[3usize][7usize] ||
    grid[4usize][0usize] || grid[4usize][1usize] || grid[4usize][2usize] || grid[4usize][3usize] ||
    grid[4usize][4usize] || grid[4usize][5usize] || grid[4usize][6usize] || grid[4usize][7usize] ||
    grid[5usize][0usize] || grid[5usize][1usize] || grid[5usize][2usize] || grid[5usize][3usize] ||
    grid[5usize][4usize] || grid[5usize][5usize] || grid[5usize][6usize] || grid[5usize][7usize] ||
    grid[6usize][0usize] || grid[6usize][1usize] || grid[6usize][2usize] || grid[6usize][3usize] ||
    grid[6usize][4usize] || grid[6usize][5usize] || grid[6usize][6usize] || grid[6usize][7usize] ||
    grid[7usize][0usize] || grid[7usize][1usize] || grid[7usize][2usize] || grid[7usize][3usize] ||
    grid[7usize][4usize] || grid[7usize][5usize] || grid[7usize][6usize] || grid[7usize][7usize]

let run = (gen, grid, limit) => {
    print_gen(gen, grid)
    if any_alive(grid) {
        if gen < limit {
            println(``)
            run(gen + 1usize, step(grid), limit)
        } else {
            println(`Stopped at generation ${to_str(gen)} (still life detected).`)
        }
    } else {
        println(`All cells died at generation ${to_str(gen)}.`)
    }
}

let main = () => {
    println(`=== Game of Life ===`)
    println(``)
    run(0usize, initial, 30usize)
}
