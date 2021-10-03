//! Support module for the maze example

use std::fmt::Write;
use std::ops::AddAssign;

use bit_vec::BitVec;
use rand::Rng;

use self::Side::{EAST, NORTH, SOUTH, WEST};

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct Cell {
    pub row: usize,
    pub col: usize,
}

impl Cell {
    fn wall(&self, side: Side, nrows: usize, ncols: usize) -> Option<Wall> {
        if side == WEST && self.col == 0
            || side == EAST && self.col == ncols - 1
            || side == NORTH && self.row == 0
            || side == SOUTH && self.row == nrows - 1 {
            None
        } else {
            Some(Wall { cell: *self, side })
        }
    }
}

pub fn cell(row: usize, col: usize) -> Cell {
    Cell { row, col }
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum Side { NORTH, EAST, SOUTH, WEST }

impl Side {
    fn opposite(&self) -> Side {
        match *self {
            NORTH => SOUTH,
            EAST => WEST,
            SOUTH => NORTH,
            WEST => EAST,
        }
    }
}

const SIDES: [Side; 4] = [Side::NORTH, Side::EAST, Side::SOUTH, Side::WEST];

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct Wall {
    pub cell: Cell,
    pub side: Side,
}

impl Wall {
    fn next_cell(&self) -> Cell {
        let Cell { row, col } = self.cell;
        match self.side {
            Side::NORTH => cell(row - 1, col),
            Side::EAST => cell(row, col + 1),
            Side::SOUTH => cell(row + 1, col),
            Side::WEST => cell(row, col - 1),
        }
    }
}

struct CellSet {
    width: usize,
    vec: BitVec,
}

impl CellSet {
    fn contains(&self, cell: &Cell) -> bool {
        self.vec[self.position_of(cell)]
    }

    fn position_of(&self, cell: &Cell) -> usize {
        cell.row + self.width * cell.col
    }

    fn set_present(&mut self, cell: &Cell, value: bool) {
        self.vec.set(self.position_of(cell), value);
    }
}

impl AddAssign<Cell> for CellSet {
    fn add_assign(&mut self, rhs: Cell) {
        self.set_present(&rhs, true);
    }
}

#[derive(Eq, PartialEq, Debug, Copy, Clone)]
pub enum PlayerPos {
    Cell(Cell),
    Wall(Wall),
}

impl PlayerPos {
    fn normalize(self) -> Self {
        match self {
            Self::Wall(wall @ Wall { side: side @ (Side::SOUTH | Side::EAST), .. }) => {
                Self::Wall(Wall { cell: wall.next_cell(), side: side.opposite() })
            }
            _ => self,
        }
    }
}

pub struct MazeModel {
    nrows: usize,
    ncols: usize,
    size: usize,
    exit: Cell,
    north_walls: CellSet,
    west_walls: CellSet,
}

const EXIT_REPR: &'static str = "$$";
const PLAYER_REPR: &'static str = "<>";

impl MazeModel {
    pub fn new(nrows: usize, ncols: usize, rng: &mut impl Rng) -> Self {
        Self {
            nrows,
            ncols,
            size: nrows * ncols,
            exit: Self::random_cell_impl(nrows, ncols, rng),
            north_walls: Self::new_cell_set_impl(nrows, ncols, true),
            west_walls: Self::new_cell_set_impl(nrows, ncols, true),
        }
    }

    fn new_cell_set_impl(nrows: usize, ncols: usize, value: bool) -> CellSet {
        CellSet { width: ncols, vec: BitVec::from_elem(nrows * ncols, value) }
    }

    fn new_cell_set(&self, value: bool) -> CellSet {
        Self::new_cell_set_impl(self.nrows, self.ncols, value)
    }

    fn random_cell(&self, rng: &mut impl Rng) -> Cell {
        Self::random_cell_impl(self.nrows, self.ncols, rng)
    }

    fn random_cell_impl(nrows: usize, ncols: usize, rng: &mut impl Rng) -> Cell {
        let row = rng.gen_range(0..nrows);
        let col = rng.gen_range(0..ncols);

        cell(row, col)
    }

    /// Mutate the position with the new position, if the move is legal.
    /// Returns whether the player has moved.
    pub fn move_to(&self, player: &mut PlayerPos, side: Side) -> bool {
        // the pos is always valid for this maze
        let new_pos = match player {
            PlayerPos::Cell(cell) =>
                cell.wall(side, self.nrows, self.ncols)
                    .filter(|wall| !self.has_wall(&wall))
                    .map(PlayerPos::Wall),
            PlayerPos::Wall(wall) => {
                debug_assert!(!self.has_wall(&wall), "player is on a wall");
                if wall.side == side {
                    Some(PlayerPos::Cell(wall.next_cell()))
                } else if wall.side == side.opposite() {
                    Some(PlayerPos::Cell(wall.cell))
                } else {
                    None
                }
            }
        };

        match new_pos {
            None => false,
            Some(pos) => {
                *player = pos;
                true
            }
        }
    }

    pub fn render_to_console(&self, player: PlayerPos) {
        use std::io::Write;
        let str = self.render_to_string_raw(player);
        let stdout = std::io::stdout();
        let mut stdout = stdout.lock();

        // this escape char clears the terminal
        write!(stdout, "\x1B[2J{}", str).unwrap();
        stdout.flush().unwrap();
    }


    pub fn render_to_string_raw(&self, player: PlayerPos) -> String {
        use std::io::Write;
        use termcolor::{Buffer, BufferWriter, Color, ColorChoice, ColorSpec, WriteColor};

        let player = player.normalize();

        let black = {
            let mut it = ColorSpec::new();
            it.set_bg(Some(Color::Black));
            it
        };
        let free = ColorSpec::new();

        let bufwtr = BufferWriter::stdout(ColorChoice::Always);
        let mut buffer = bufwtr.buffer();
        let mut hline = bufwtr.buffer();
        let mut vline = bufwtr.buffer();

        fn write_px(buf: &mut Buffer, color: &ColorSpec) {
            buf.set_color(color);
            write!(buf, "   ");
            buf.set_color(&ColorSpec::new());
        }

        fn write_px_with_dude(buf: &mut Buffer) {
            buf.set_color(&ColorSpec::new());
            write!(buf, " I ");
        }

        for row in 0..self.nrows {
            for buf in [&mut hline, &mut vline] {
                buf.clear();
                write_px(buf, &free);
            }

            for col in 0..self.ncols {
                let cell = cell(row, col);

                write_px(&mut hline, &black);
                if self.north_walls.contains(&cell) {
                    write_px(&mut hline, &black);
                } else if player == PlayerPos::Wall(Wall { cell, side: NORTH }) {
                    write_px_with_dude(&mut hline);
                } else {
                    write_px(&mut hline, &free);
                }

                if self.west_walls.contains(&cell) {
                    write_px(&mut vline, &black);
                } else if player == PlayerPos::Wall(Wall { cell, side: WEST }) {
                    write_px_with_dude(&mut vline);
                } else {
                    write_px(&mut vline, &free);
                }

                if player == PlayerPos::Cell(cell) {
                    write_px_with_dude(&mut vline);
                } else {
                    write_px(&mut vline, &free);
                }
            }

            buffer.write_all(hline.as_slice());
            write_px(&mut buffer, &black);
            write!(buffer, "\n\r");
            buffer.write_all(vline.as_slice());
            write_px(&mut buffer, &black);
            write!(buffer, "\n\r");
        }
        write_px(&mut buffer, &free);

        for _ in 0..self.ncols {
            write_px(&mut buffer, &black);
            write_px(&mut buffer, &black);
        }
        write_px(&mut buffer, &black);
        write!(buffer, "\n\r");

        String::from_utf8(buffer.into_inner()).unwrap()
    }

    pub fn display(&self, out: &mut impl Write, player: Cell) -> std::fmt::Result {
        let mut hline = String::with_capacity(self.ncols);
        let mut vline = String::with_capacity(self.ncols);

        hline += "   ";
        vline += "   ";

        for row in 0..self.nrows {
            hline.truncate(3); // reset to "   "
            vline.truncate(3); // reset to "   "

            for col in 0..self.ncols {
                let cell = cell(row, col);
                hline += if self.north_walls.contains(&cell) { "+--" } else { "+  " };
                vline += if self.west_walls.contains(&cell) { "|" } else { " " };
                vline +=
                    if self.exit == cell { EXIT_REPR } else if player == cell { PLAYER_REPR } else { "  " }
            }
            write!(out, "{}+\n", hline)?;
            write!(out, "{}|\n", vline)?;
        }
        write!(out, "   ")?;
        for _ in 0..self.ncols {
            write!(out, "+--")?;
        }

        write!(out, "+")
    }

    fn contains(&self, cell: Cell) -> bool {
        (0..self.nrows).contains(&cell.row)
            && (0..self.ncols).contains(&cell.col)
    }


    fn set_wall(&mut self, wall: Wall, on: bool) {
        debug_assert!(self.contains(wall.cell));
        if !self.contains(wall.next_cell()) {
            return;
        }

        match wall.side {
            Side::NORTH => self.north_walls.set_present(&wall.cell, on),
            Side::SOUTH => self.north_walls.set_present(&wall.next_cell(), on),
            Side::WEST => self.west_walls.set_present(&wall.cell, on),
            Side::EAST => self.west_walls.set_present(&wall.next_cell(), on),
        }
    }

    fn break_wall(&mut self, wall: Wall) {
        self.set_wall(wall, false)
    }

    fn has_wall(&self, wall: &Wall) -> bool {
        debug_assert!(self.contains(wall.cell));
        if !self.contains(wall.next_cell()) {
            return true;
        }

        match wall.side {
            Side::NORTH => self.north_walls.contains(&wall.cell),
            Side::SOUTH => self.north_walls.contains(&wall.next_cell()),
            Side::WEST => self.west_walls.contains(&wall.cell),
            Side::EAST => self.west_walls.contains(&wall.next_cell()),
        }
    }

    fn walls_around(&self, cell: Cell) -> impl Iterator<Item=Wall> + '_ {
        SIDES.iter().filter_map(move |side| cell.wall(*side, self.nrows, self.ncols))
    }

    pub fn generate_prim(&mut self, rng: &mut impl Rng) {
        let mut visited = self.new_cell_set(false);
        let seed = self.random_cell(rng);
        visited += seed;
        let mut walls = self.walls_around(seed).collect::<Vec<_>>();

        while !walls.is_empty() {
            let wall = walls.remove(rng.gen_range(0..walls.len()));

            if visited.contains(&wall.cell) ^ visited.contains(&wall.next_cell()) {
                self.break_wall(wall);

                visited += wall.next_cell();
                for new_wall in self.walls_around(wall.next_cell()) {
                    if !walls.contains(&new_wall) {
                        walls.push(new_wall);
                    }
                }
            }
        }
    }
}
#[cfg(test)]
mod test {
    use rand::SeedableRng;
    use rand::rngs::StdRng;
    use crate::{cell, Maze};

    use super::*;

    #[test]
    pub fn test_render_fully_walled() {
        let maze = MazeModel::new(5, 5, &mut StdRng::seed_from_u64(0));
        let mut str = String::new();
        maze.display(&mut str, cell(3, 4));
        assert_eq!(str.as_str(),
                   r#"   +--+--+--+--+--+
   |  |  |  |  |  |
   +--+--+--+--+--+
   |  |  |  |  |  |
   +--+--+--+--+--+
   |  |  |  |$$|  |
   +--+--+--+--+--+
   |  |  |  |  |<>|
   +--+--+--+--+--+
   |  |  |  |  |  |
   +--+--+--+--+--+"#
        );
    }

    #[test]
    pub fn test_render_prim() {
        let mut rng = StdRng::seed_from_u64(1);
        let mut maze = MazeModel::new(10, 10, &mut rng);
        maze.generate_prim(&mut rng);
        let mut str = String::new();
        maze.display(&mut str, cell(6, 4));
        maze.render_to_console();
        assert_eq!(str.as_str(),
                   r#"
   +--+--+--+--+--+--+--+--+--+--+
   |  |  |  |                 |  |
   +  +  +  +--+--+  +--+--+--+  +
   |     |        |  |  |  |     |
   +--+  +--+--+  +  +  +  +  +--+
   |        |     |     |     |  |
   +--+--+  +--+  +--+  +--+  +  +
   |  |  |  |        |  |        |
   +  +  +  +  +--+--+  +  +--+--+
   |      $$   |     |  |        |
   +--+  +--+  +--+  +  +  +--+--+
   |  |  |        |  |           |
   +  +--+--+  +  +  +  +--+--+--+
   |  |        |<>         |     |
   +  +--+--+--+--+  +  +--+  +--+
   |           |     |           |
   +  +--+--+  +  +  +  +  +  +  +
   |  |     |     |  |  |  |  |  |
   +--+--+  +  +--+  +--+  +  +  +
   |           |        |  |  |  |
   +--+--+--+--+--+--+--+--+--+--+"#
        );
    }
}
