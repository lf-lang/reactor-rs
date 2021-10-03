//! Support module for the snake game example.


/// Position on the grid
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
struct Cell { row: usize, col: usize }

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum Direction { UP, RIGHT, DOWN, LEFT }

impl Cell {
    fn wrap_add(a: usize, add: bool, grid_side: usize) -> usize {
        if a == grid_side - 1 && add { 0 }
        else if a == 0 && !add { grid_side - 1 }
        else if add { a + 1 }
        else { a - 1 }
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

    /// Mutate internal state.
    /// Returns true if we could advance. Returns false if the
    /// move is illegal -> we've lost.
    pub fn advance(&mut self, direction: Direction, just_ate_something: bool) -> bool {
        let head = &self.snake_positions[self.head];
        let new_head = head.shift(direction, self.grid_side);

        if self.snake_positions.contains(&new_head) {
            // we're eating our tail, move is illegal
            return false;
        }

        if just_ate_something {
            // then the tail is not moving, we increase size of the snake
            self.snake_positions.insert(self.head + 1, new_head);
            self.head = self.head + 1;
        } else {
            // replace old tail with new head, shift head ptr wrapping around
            let tail = (self.head + 1) % self.snake_positions.len();
            self.snake_positions[tail] = new_head;
            self.head = tail;
        }
        true
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum CellState {
    Snake,
    SnakeHead,
    SnakeTail,
    Food,
    Free,
}

struct SnakeWorld {
    grid: Vec<CellState>,
    grid_side: usize,
    snake: CircularSnake,
}

impl SnakeWorld {
    fn new(grid_side: usize) -> Self {
        Self {
            grid: vec![CellState::Free; grid_side],
            grid_side,
            snake: CircularSnake::new(grid_side)
        }
    }

    fn step_and_repaint(&mut self, snake_heading: Direction) -> bool {
        for i in 0..self.grid_side {
            for j in 0..self.grid_side {
                match self.grid[i * self.grid_side + j] {
                    CellState::Snake => {

                    }
                    CellState::SnakeHead => {}
                    CellState::SnakeTail => {}
                    CellState::Food => {

                    }
                    CellState::Free => {

                    }
                }
            }
        }

        todo!()
    }
}
