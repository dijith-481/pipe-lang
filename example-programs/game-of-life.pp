// Conway's Game of Life — functional cellular automata
use stdlib.io

type Cell = bool

let step : (Array<Array<Cell>>) -> Array<Array<Cell>>
let step = (grid) => {
    let h = grid.len()
    let w = grid[0].len()

    // Count neighbors for a cell at (row, col)
    let countNeighbors = (row, col) => {
        let offsets = [-1, 0, 1]
        let count = 0
        offsets.fold(count, (acc, dr) =>
            offsets.fold(acc, (acc2, dc) => {
                let r = row + dr
                let c = col + dc
                if r >= 0 && r < h && c >= 0 && c < w && !(dr == 0 && dc == 0) {
                    if grid[r][c] { acc2 + 1 } else { acc2 }
                } else {
                    acc2
                }
            })
        )
    }

    // Apply rules
    let rows = [0, 1, 2, 3, 4]  // placeholder indices
    rows.map((r) => {
        let cols = [0, 1, 2, 3, 4]
        cols.map((c) => {
            let neighbors = countNeighbors(r, c)
            let alive = grid[r][c]
            if alive {
                neighbors == 2 || neighbors == 3
            } else {
                neighbors == 3
            }
        })
    })
}

let cellToString : (Cell) -> str
let cellToString = (c) => if c { "#" } else { "." }

let gridToString : (Array<Array<Cell>>) -> str
let gridToString = (grid) =>
    grid.map((row) =>
        row.map(cellToString).fold("", (acc, s) => acc ++ s)
    ).fold("", (acc, line) => acc ++ line ++ "\n")

let main : () -> Effect<Unit>
let main = do {
    // Glider pattern
    let grid = [
        [false, true,  false, false, false, false, false, false],
        [false, false, true,  false, false, false, false, false],
        [true,  true,  true,  false, false, false, false, false],
        [false, false, false, false, false, false, false, false],
        [false, false, false, false, false, false, false, false],
        [false, false, false, false, false, false, false, false],
        [false, false, false, false, false, false, false, false],
        [false, false, false, false, false, false, false, false],
    ]

    IO.println("Generation 0:")
    IO.println(gridToString(grid))

    // Step a few times
    let g1 = step(grid)
    IO.println("Generation 1:")
    IO.println(gridToString(g1))
}
