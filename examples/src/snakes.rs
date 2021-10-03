//! Support module for the snake game example.


use std::ops::{Index, IndexMut};


/// Position on the grid
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
struct Cell {
    row: usize,
    col: usize,
}

pub fn cell(row: usize, col: usize) -> Cell {
    Cell { row, col }
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum Direction { UP, RIGHT, DOWN, LEFT }

impl Cell {
    fn wrap_add(a: usize, add: bool, grid_side: usize) -> usize {
        if a == grid_side - 1 && add { 0 } else if a == 0 && !add { grid_side - 1 } else if add { a + 1 } else { a - 1 }
    }

    /// Returns the neighbor cell that's in the given direction,
    /// wrap around the grid if needed.
    fn shift(self, direction: Direction, grid_side: usize) -> Cell {
        let Cell { row, col } = self;
        match direction {
            Direction::UP => Cell { row: Self::wrap_add(row, false, grid_side), col },
            Direction::RIGHT => Cell { row, col: Self::wrap_add(col, true, grid_side) },
            Direction::DOWN => Cell { row: Self::wrap_add(row, true, grid_side), col },
            Direction::LEFT => Cell { row, col: Self::wrap_add(col, false, grid_side) },
        }
    }
}

struct CircularSnake {
    /// Circular buffer, the head field is the first element,
    /// then they're in order when you read from right to left.
    /// ```no_compile
    ///    [3, 2, 1]
    ///           ^head
    /// ```
    /// This makes advancing while preserving order a constant
    /// time/space operation. But growing the snake is linear.
    /// Using a linked list would make it constant.
    ///
    /// Whatever it's a snake lol
    snake_positions: Vec<Cell>,
    head: usize,
    /// Side of the square grid
    grid_side: usize,
}

impl CircularSnake {
    pub fn new(grid_side: usize) -> Self {
        Self {
            snake_positions: vec![Cell { row: grid_side / 2, col: grid_side / 2 }],
            head: 0,
            grid_side,
        }
    }

    fn head(&self) -> Cell {
        self.snake_positions[self.head]
    }

    fn len(&self) -> usize {
        self.snake_positions.len()
    }

    /// Mutate internal state of the grid too.
    /// Returns cells that have changed. If none, the move was illegal
    pub fn advance(&mut self, snake_heading: Direction, grid: &mut SnakeGrid) -> bool {
        let head = &self.snake_positions[self.head];
        let new_head = head.shift(snake_heading, self.grid_side);
        match grid[new_head] {
            // we're eating our tail, move is illegal
            CellState::Snake => false,
            CellState::Food => {
                // then the tail is not moving, we increase size of the snake
                self.snake_positions.insert(self.head + 1, new_head);
                self.head = self.head + 1;
                grid[new_head] = CellState::Snake;
                true
            }
            CellState::Free => {
                // replace old tail with new head, shift head ptr wrapping around
                let tail = (self.head + 1) % self.snake_positions.len();
                let old_tail = self.snake_positions[tail];
                self.snake_positions[tail] = new_head;
                self.head = tail;

                grid[old_tail] = CellState::Free;
                grid[new_head] = CellState::Snake;
                true
            }
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum CellState {
    Snake,
    Food,
    Free,
}

struct SnakeGrid {
    grid: Vec<CellState>,
    grid_side: usize,
    snake: CircularSnake,
}

impl Index<Cell> for SnakeGrid {
    type Output = CellState;

    fn index(&self, cell: Cell) -> &Self::Output {
        &self.grid[self.index_of(cell)]
    }
}

impl IndexMut<Cell> for SnakeGrid {
    fn index_mut(&mut self, cell: Cell) -> &mut Self::Output {
        let i = self.index_of(cell);
        &mut self.grid[i]
    }
}

impl SnakeGrid {
    fn new(grid_side: usize) -> Self {
        Self {
            grid_side,
            grid: vec![CellState::Free; grid_side],
            snake: CircularSnake::new(grid_side),
        }
    }

    pub fn grid_side(&self) -> usize {
        self.grid_side
    }

    fn index_of(&self, cell: Cell) -> usize {
        cell.row * self.grid_side + cell.col
    }

    pub fn step(&mut self, snake_heading: Direction) -> bool {
        self.snake.advance(snake_heading, self)
    }
}


pub fn paint_on_raw_console(grid: &SnakeGrid) {
    use std::io::Write;
    let str = format_for_raw_console(grid);
    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();

    // this escape char clears the terminal
    write!(stdout, "\x1B[2J{}", str).unwrap();
    stdout.flush().unwrap();
}

fn format_for_raw_console(grid: &SnakeGrid) -> String {
    use std::io::Write;
    use termcolor::{Buffer, BufferWriter, Color, ColorChoice, ColorSpec, WriteColor};

    let bufwtr = BufferWriter::stdout(ColorChoice::Always);
    let mut buf = bufwtr.buffer();

    for _ in 0..grid.grid_side() {
        write!(&mut buf, "-")
    }
    write!(&mut buf, "|\n\r");

    for row in 0..grid.grid_side() {
        write!(&mut buf, "|");
        for col in 0..grid.grid_side() {
            match grid[cell(row, col)] {
                CellState::Snake =>
                    if grid.snake.head() == cell(row, col) {
                        write!(&mut buf, "@");
                    } else {
                        write!(&mut buf, "o");
                    },
                CellState::Food => write!(&mut buf, "$"),
                CellState::Free => write!(&mut buf, " "),
            }
        }
        write!(&mut buf, "|\n\r");
    }
    for _ in 0..grid.grid_side() {
        write!(&mut buf, "-")
    }

    String::from_utf8(buffer.into_inner()).unwrap()
}
