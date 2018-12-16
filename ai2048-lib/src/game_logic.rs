//! 2048 game logic is implemented here.
use lazy_static::lazy_static;
use rand::{self, Rng};
use std::{fmt, u16};

/// Represents a move.
#[derive(Eq, PartialEq, Hash, Copy, Clone, Debug)]
#[repr(u8)]
pub enum Move {
    /// Move left.
    Left = 0,
    /// Move right.
    Right = 1,
    /// Move up.
    Up = 2,
    /// Move down.
    Down = 3,
}

/// All possible moves.
pub const MOVES: [Move; 4] = [Move::Left, Move::Right, Move::Up, Move::Down];

impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Move::Down => "Down".fmt(f),
            Move::Left => "Left".fmt(f),
            Move::Right => "Right".fmt(f),
            Move::Up => "Up".fmt(f),
        }
    }
}

#[derive(Eq, PartialEq, Hash, Copy, Clone, Default)]
pub(crate) struct Row(pub(crate) u16);

impl fmt::Debug for Row {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let unpacked = self.unpack();
        write!(
            f,
            "[{:0>4b} {:0>4b} {:0>4b} {:0>4b}]",
            unpacked[0], unpacked[1], unpacked[2], unpacked[3]
        )
    }
}

impl Row {
    pub(crate) fn pack(row: [u8; 4]) -> Option<Row> {
        let mut result = 0;
        for &tile in &row {
            if tile > 0b1111 {
                return None;
            }
            result <<= 4;
            result += u16::from(tile);
        }
        Some(Row(result))
    }

    pub(crate) fn unpack(self) -> [u8; 4] {
        let row = self.0;
        let tile0 = ((row & 0b1111_0000_0000_0000) >> 12) as u8;
        let tile1 = ((row & 0b0000_1111_0000_0000) >> 8) as u8;
        let tile2 = ((row & 0b0000_0000_1111_0000) >> 4) as u8;
        let tile3 = (row & 0b0000_0000_0000_1111) as u8;
        [tile0, tile1, tile2, tile3]
    }

    fn reverse(self) -> Self {
        Row((self.0 >> 12)
            | ((self.0 >> 4) & 0b0000_0000_1111_0000)
            | ((self.0 << 4) & 0b0000_1111_0000_0000)
            | (self.0 << 12))
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct Column(u64);

impl Column {
    fn from_row(row: Row) -> Self {
        const COLUMN_MASK: u64 = 0x000F_000F_000F_000F;
        let col = (u64::from(row.0)
            | u64::from(row.0) << 12
            | u64::from(row.0) << 24
            | u64::from(row.0) << 36)
            & COLUMN_MASK;
        Column(col)
    }
}

/// `Grid`, in general, encodes all the rules of the game: it can generate new states
/// given a move a player makes, or all possible states following the computer spawning a random
/// tile.
#[derive(Eq, PartialEq, Hash, Copy, Clone, Default)]
pub struct Grid(u64);

impl fmt::Display for Grid {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for row in self.unpack_human().iter() {
            for &tile in row {
                write!(f, "{number:>width$}", number = tile, width = 6)?;
            }
            writeln!(f)?;
        }

        Ok(())
    }
}

impl fmt::Debug for Grid {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for row in self.rows().iter() {
            write!(f, "{:?} ", row)?;
        }

        Ok(())
    }
}

fn to_log(n: u32) -> Option<u8> {
    use std::f32;

    let log = match n {
        0 => 0f32,
        _ => (n as f32).log2(),
    };

    let rounded = log.round();
    if (rounded - log) < 1e-10 {
        Some(rounded as u8)
    } else {
        None
    }
}

impl Grid {
    /// Creates a new `Grid` from an array of human-looking numbers. If a tile fails to be
    /// a power of 2, or is larger than 32768, returns `None`.
    pub fn from_human(grid: [[u32; 4]; 4]) -> Option<Grid> {
        let mut rows = [Row::default(); 4];
        for (x, &row) in grid.iter().enumerate() {
            let mut new_row = [0u8; 4];
            for (y, &tile) in row.iter().enumerate() {
                let log = to_log(tile)?;
                new_row[y] = log;
            }

            rows[x] = Row::pack(new_row)?;
        }
        Some(Grid::from_rows(rows))
    }

    /// Unpacks a human-readable representation from `Grid`'s internal representation
    pub fn unpack_human(self) -> [[u32; 4]; 4] {
        let mut result = [[0; 4]; 4];
        let grid_u8 = self.unpack_log();
        for (x, row) in grid_u8.iter().enumerate() {
            for (y, &tile) in row.iter().enumerate() {
                result[x][y] = match tile {
                    0 => 0,
                    _ => 1 << tile,
                };
            }
        }
        result
    }

    fn from_log(grid: [[u8; 4]; 4]) -> Option<Grid> {
        let mut rows = [Row::default(); 4];
        for (x, &row) in grid.iter().enumerate() {
            rows[x] = Row::pack(row)?;
        }
        Some(Grid::from_rows(rows))
    }

    fn unpack_log(self) -> [[u8; 4]; 4] {
        let mut result = [[0; 4]; 4];
        for (x, row) in self.rows().iter().enumerate() {
            result[x] = row.unpack();
        }
        result
    }

    pub(crate) fn rows(self) -> [Row; 4] {
        let row1 = Row(((self.0 & 0xFFFF_0000_0000_0000) >> 48) as u16);
        let row2 = Row(((self.0 & 0x0000_FFFF_0000_0000) >> 32) as u16);
        let row3 = Row(((self.0 & 0x0000_0000_FFFF_0000) >> 16) as u16);
        let row4 = Row((self.0 & 0x0000_0000_0000_FFFF) as u16);
        [row1, row2, row3, row4]
    }

    fn from_rows(rows: [Row; 4]) -> Self {
        let mut grid = Grid::default();
        grid.0 |= u64::from(rows[0].0) << 48;
        grid.0 |= u64::from(rows[1].0) << 32;
        grid.0 |= u64::from(rows[2].0) << 16;
        grid.0 |= u64::from(rows[3].0);
        grid
    }

    fn from_columns(columns: [Column; 4]) -> Self {
        let mut grid = Grid::default();
        grid.0 |= columns[0].0 << 12;
        grid.0 |= columns[1].0 << 8;
        grid.0 |= columns[2].0 << 4;
        grid.0 |= columns[3].0;
        grid
    }

    pub fn game_over(self) -> bool {
        MOVES.iter().find(|&&m| self.make_move(m) != self).is_none()
    }

    /// Creates a new `Grid` with a random tile (10% of times a `2`, 90% of times a `4`) added to a
    /// random empty tile on the grid.
    pub fn add_random_tile(self) -> Grid {
        let mut rng = rand::thread_rng();

        let mut grid = self.unpack_log();
        let empty_tile_count = grid.iter().flatten().filter(|v| **v == 0).count();
        let position = rng.gen_range(0, empty_tile_count);

        let value = grid
            .iter_mut()
            .flatten()
            .filter(|v| **v == 0)
            .nth(position)
            .unwrap();

        *value = if rng.gen_bool(0.1) { 2 } else { 1 };

        Grid::from_log(grid).unwrap()
    }

    pub(crate) fn ai_moves_with2(self) -> impl Iterator<Item = Grid> {
        AiMoves::new(self, 1)
    }

    pub(crate) fn ai_moves_with4(self) -> impl Iterator<Item = Grid> {
        AiMoves::new(self, 2)
    }

    pub(crate) fn player_moves(self) -> impl Iterator<Item = (Move, Grid)> {
        MOVES.iter().filter_map(move |&m| {
            let new_grid = self.make_move(m);
            if new_grid == self {
                None
            } else {
                Some((m, new_grid))
            }
        })
    }

    pub(crate) fn transpose(self) -> Grid {
        let x = self.0;
        let a1 = x & 0xF0F0_0F0F_F0F0_0F0F;
        let a2 = x & 0x0000_F0F0_0000_F0F0;
        let a3 = x & 0x0F0F_0000_0F0F_0000;
        let a = a1 | (a2 << 12) | (a3 >> 12);
        let b1 = a & 0xFF00_FF00_00FF_00FF;
        let b2 = a & 0x00FF_00FF_0000_0000;
        let b3 = a & 0x0000_0000_FF00_FF00;
        let ret = b1 | (b2 >> 24) | (b3 << 24);
        Grid(ret)
    }

    pub(crate) fn count_empty(self) -> usize {
        let mut x = self.0;
        x |= (x >> 2) & 0x3333333333333333;
        x |= x >> 1;
        x = (!x) & 0x1111111111111111;
        // At this point each nibble is:
        //  0 if the original nibble was non-zero
        //  1 if the original nibble was zero
        // Next sum them all
        x += x >> 32;
        x += x >> 16;
        x += x >> 8;
        x += x >> 4; // this can overflow to the next nibble if there were 16 empty positions
        return (x & 0xf) as usize;
    }

    /// Returns a `Grid` that would result from making a certain `Move` in the current state.
    pub fn make_move(self, mv: Move) -> Grid {
        match mv {
            Move::Left => self.move_left(),
            Move::Right => self.move_right(),
            Move::Up => self.move_up(),
            Move::Down => self.move_down(),
        }
    }

    fn move_left(self) -> Grid {
        let rows = self.rows();
        let row0 = lookup_left(rows[0]);
        let row1 = lookup_left(rows[1]);
        let row2 = lookup_left(rows[2]);
        let row3 = lookup_left(rows[3]);
        Grid::from_rows([row0, row1, row2, row3])
    }

    fn move_right(self) -> Grid {
        let rows = self.rows();
        let row0 = lookup_right(rows[0]);
        let row1 = lookup_right(rows[1]);
        let row2 = lookup_right(rows[2]);
        let row3 = lookup_right(rows[3]);
        Grid::from_rows([row0, row1, row2, row3])
    }

    fn move_up(self) -> Grid {
        let rows = self.transpose().rows();
        let col0 = lookup_up(rows[0]);
        let col1 = lookup_up(rows[1]);
        let col2 = lookup_up(rows[2]);
        let col3 = lookup_up(rows[3]);
        Grid::from_columns([col0, col1, col2, col3])
    }

    fn move_down(self) -> Grid {
        let rows = self.transpose().rows();
        let col0 = lookup_down(rows[0]);
        let col1 = lookup_down(rows[1]);
        let col2 = lookup_down(rows[2]);
        let col3 = lookup_down(rows[3]);
        Grid::from_columns([col0, col1, col2, col3])
    }

    pub(crate) fn count_distinct_tiles(self) -> usize {
        let mut grid = self.0;
        let mut bitset = 0u16;
        while grid != 0 {
            bitset |= 1 << (grid & 0xF);
            grid >>= 4;
        }
        bitset >>= 1;
        let mut count = 0;
        while bitset != 0 {
            bitset &= bitset - 1;
            count += 1;
        }

        count
    }
}

struct AiMoves {
    grid: Grid,
    index: i8,
    val: u8,
}

impl AiMoves {
    fn new(grid: Grid, new_value: u8) -> AiMoves {
        AiMoves {
            grid,
            index: 16,
            val: new_value,
        }
    }
}

impl Iterator for AiMoves {
    type Item = Grid;

    fn next(&mut self) -> Option<Grid> {
        loop {
            self.index -= 1;
            if self.index < 0 {
                return None;
            }
            let mask = 0b1111u64 << (self.index * 4);
            if (self.grid.0 & mask) == 0 {
                let grid = Grid(self.grid.0 | u64::from(self.val) << (self.index * 4));
                return Some(grid);
            }
        }
    }
}

// Not much effort spent optimizing this, since it's going to be cached anyway
fn move_row_left(row: Row) -> Row {
    let from_row = row.unpack();

    let mut to_row = [0; 4];
    let mut last = 0;
    let mut last_index = 0;

    for &tile in from_row.iter() {
        if tile == 0 {
            continue;
        }

        if last == 0 {
            last = tile;
            continue;
        }

        if tile == last {
            to_row[last_index as usize] = last + 1;
            last = 0;
        } else {
            to_row[last_index as usize] = last;
            last = tile;
        }

        last_index += 1;
    }

    if last != 0 {
        to_row[last_index as usize] = last;
    }

    Row::pack(to_row).unwrap_or_default()
}

fn lookup_left(row: Row) -> Row {
    unsafe { *CACHE_LEFT.get_unchecked(row.0 as usize) }
}
fn lookup_right(row: Row) -> Row {
    unsafe { *CACHE_RIGHT.get_unchecked(row.0 as usize) }
}
fn lookup_up(row: Row) -> Column {
    unsafe { *CACHE_UP.get_unchecked(row.0 as usize) }
}
fn lookup_down(row: Row) -> Column {
    unsafe { *CACHE_DOWN.get_unchecked(row.0 as usize) }
}

lazy_static! {
    static ref CACHE_LEFT: Box<[Row]> = {
        let mut vec = vec![Row::default(); u16::MAX as usize];
        for (index, row) in vec.iter_mut().enumerate() {
            *row = move_row_left(Row(index as u16));
        }
        vec.into()
    };
    static ref CACHE_RIGHT: Box<[Row]> = {
        let mut vec = vec![Row::default(); u16::MAX as usize];
        for (index, row) in vec.iter_mut().enumerate() {
            *row = move_row_left(Row(index as u16).reverse()).reverse();
        }
        vec.into()
    };
    static ref CACHE_UP: Box<[Column]> = {
        let mut vec = vec![Column::default(); u16::MAX as usize];
        for (index, col) in vec.iter_mut().enumerate() {
            *col = Column::from_row(CACHE_LEFT[index]);
        }
        vec.into()
    };
    static ref CACHE_DOWN: Box<[Column]> = {
        let mut vec = vec![Column::default(); u16::MAX as usize];
        for (index, col) in vec.iter_mut().enumerate() {
            *col = Column::from_row(CACHE_RIGHT[index]);
        }
        vec.into()
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_create_empty_grid() {
        let expected =
            Grid::from_human([[0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0]]).unwrap();

        let actual = Grid::default();

        assert_eq!(expected, actual);
    }

    #[test]
    fn can_create_grid_from_human_input() {
        let human = [
            [0, 2, 4, 8],
            [16, 32, 64, 128],
            [256, 512, 1024, 2048],
            [4096, 8192, 16384, 32768],
        ];

        let actual = Grid::from_human(human);

        assert!(actual.is_some());
        assert_eq!(human, actual.unwrap().unpack_human());
    }

    #[test]
    fn can_return_none_on_invalid_input() {
        let result =
            Grid::from_human([[0, 1, 2, 3], [4, 5, 6, 7], [8, 9, 10, 11], [12, 13, 14, 15]]);

        assert!(result.is_none());
    }

    #[test]
    fn can_add_random_tile() {
        for _ in 0..1000 {
            let mut grid = Grid::default();
            for _ in 0..8 {
                grid = grid.add_random_tile();
            }

            let count = grid
                .unpack_log()
                .iter()
                .flatten()
                .filter(|&&v| v == 1 || v == 2)
                .count();

            assert_eq!(8, count);
        }
    }

    #[test]
    fn can_to_string() {
        let grid = Grid::from_human([
            [0, 2, 4, 8],
            [16, 32, 64, 128],
            [256, 512, 1024, 2048],
            [4096, 8192, 16384, 32768],
        ])
        .unwrap();

        let mut expected = String::new();
        expected.push_str("     0     2     4     8\n");
        expected.push_str("    16    32    64   128\n");
        expected.push_str("   256   512  1024  2048\n");
        expected.push_str("  4096  8192 16384 32768\n");

        let actual = grid.to_string();

        assert_eq!(expected, actual);
    }

    #[test]
    fn can_make_move_left() {
        let grid =
            Grid::from_human([[2, 2, 4, 4], [0, 2, 2, 0], [0, 2, 2, 2], [2, 0, 0, 2]]).unwrap();
        let expected =
            Grid::from_human([[4, 8, 0, 0], [4, 0, 0, 0], [4, 2, 0, 0], [4, 0, 0, 0]]).unwrap();

        let actual = grid.make_move(Move::Left);

        assert_eq!(expected, actual);
    }

    #[test]
    fn can_make_move_right() {
        let grid =
            Grid::from_human([[2, 2, 4, 4], [0, 2, 2, 0], [0, 2, 2, 2], [2, 0, 0, 2]]).unwrap();
        let expected =
            Grid::from_human([[0, 0, 4, 8], [0, 0, 0, 4], [0, 0, 2, 4], [0, 0, 0, 4]]).unwrap();

        let actual = grid.make_move(Move::Right);

        assert_eq!(expected, actual);
    }

    #[test]
    fn can_make_move_up() {
        let grid =
            Grid::from_human([[2, 2, 4, 4], [0, 2, 2, 0], [0, 2, 2, 2], [2, 0, 0, 2]]).unwrap();
        let expected =
            Grid::from_human([[4, 4, 4, 4], [0, 2, 4, 4], [0, 0, 0, 0], [0, 0, 0, 0]]).unwrap();

        let actual = grid.make_move(Move::Up);

        assert_eq!(expected, actual);
    }

    #[test]
    fn can_make_move_down() {
        let grid =
            Grid::from_human([[2, 2, 4, 4], [0, 2, 2, 0], [0, 2, 2, 2], [2, 0, 0, 2]]).unwrap();
        let expected =
            Grid::from_human([[0, 0, 0, 0], [0, 0, 0, 0], [0, 2, 4, 4], [4, 4, 4, 4]]).unwrap();

        let actual = grid.make_move(Move::Down);

        assert_eq!(expected, actual);
    }

    #[test]
    fn can_possible_grids_with2() {
        let grid =
            Grid::from_human([[0, 8, 8, 8], [8, 8, 0, 8], [8, 8, 8, 0], [8, 0, 8, 8]]).unwrap();

        let expected = vec![
            Grid::from_human([[2, 8, 8, 8], [8, 8, 0, 8], [8, 8, 8, 0], [8, 0, 8, 8]]).unwrap(),
            Grid::from_human([[0, 8, 8, 8], [8, 8, 2, 8], [8, 8, 8, 0], [8, 0, 8, 8]]).unwrap(),
            Grid::from_human([[0, 8, 8, 8], [8, 8, 0, 8], [8, 8, 8, 2], [8, 0, 8, 8]]).unwrap(),
            Grid::from_human([[0, 8, 8, 8], [8, 8, 0, 8], [8, 8, 8, 0], [8, 2, 8, 8]]).unwrap(),
        ];

        let actual = grid.ai_moves_with2().collect::<Vec<_>>();

        assert_eq!(expected, actual);
    }

    #[test]
    fn can_possible_grids_with4() {
        let grid =
            Grid::from_human([[0, 8, 8, 8], [8, 8, 0, 8], [8, 8, 8, 0], [8, 0, 8, 8]]).unwrap();

        let expected = vec![
            Grid::from_human([[4, 8, 8, 8], [8, 8, 0, 8], [8, 8, 8, 0], [8, 0, 8, 8]]).unwrap(),
            Grid::from_human([[0, 8, 8, 8], [8, 8, 4, 8], [8, 8, 8, 0], [8, 0, 8, 8]]).unwrap(),
            Grid::from_human([[0, 8, 8, 8], [8, 8, 0, 8], [8, 8, 8, 4], [8, 0, 8, 8]]).unwrap(),
            Grid::from_human([[0, 8, 8, 8], [8, 8, 0, 8], [8, 8, 8, 0], [8, 4, 8, 8]]).unwrap(),
        ];

        let actual = grid.ai_moves_with4().collect::<Vec<_>>();

        assert_eq!(expected, actual);
    }

    #[test]
    fn can_make_player_moves() {
        let grid =
            Grid::from_human([[0, 0, 0, 2], [0, 2, 0, 2], [4, 0, 0, 2], [0, 0, 0, 2]]).unwrap();

        let mut player_moves = grid.player_moves();

        assert_eq!(
            Some((
                Move::Left,
                Grid::from_human([[2, 0, 0, 0], [4, 0, 0, 0], [4, 2, 0, 0], [2, 0, 0, 0]]).unwrap()
            )),
            player_moves.next()
        );
        assert_eq!(
            Some((
                Move::Right,
                Grid::from_human([[0, 0, 0, 2], [0, 0, 0, 4], [0, 0, 4, 2], [0, 0, 0, 2],])
                    .unwrap()
            )),
            player_moves.next()
        );
        assert_eq!(
            Some((
                Move::Up,
                Grid::from_human([[4, 2, 0, 4], [0, 0, 0, 4], [0, 0, 0, 0], [0, 0, 0, 0],])
                    .unwrap()
            )),
            player_moves.next()
        );
        assert_eq!(
            Some((
                Move::Down,
                Grid::from_human([[0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 4], [4, 2, 0, 4],])
                    .unwrap()
            )),
            player_moves.next()
        );
        assert_eq!(None, player_moves.next());
    }

    #[test]
    fn can_detect_terminal_state() {
        let terminal_grid =
            Grid::from_human([[4, 16, 8, 4], [8, 128, 32, 2], [2, 32, 16, 8], [4, 2, 4, 2]])
                .unwrap();
        let normal_grid =
            Grid::from_human([[0, 8, 8, 8], [8, 8, 0, 8], [8, 8, 8, 0], [8, 0, 8, 8]]).unwrap();

        assert!(terminal_grid.game_over());
        assert!(!normal_grid.game_over());
    }

    #[test]
    fn can_transpose() {
        let grid = Grid::from_human([
            [1, 2, 4, 8],
            [16, 32, 64, 128],
            [256, 512, 1024, 2048],
            [4096, 8192, 16384, 32768],
        ])
        .unwrap();
        let expected = Grid::from_human([
            [1, 16, 256, 4096],
            [2, 32, 512, 8192],
            [4, 64, 1024, 16384],
            [8, 128, 2048, 32768],
        ])
        .unwrap();
        let transposed = grid.transpose();
        assert_eq!(transposed, expected);
    }

    #[test]
    fn can_to_rows_from_rows() {
        let grid = Grid::from_human([
            [1, 2, 4, 8],
            [16, 32, 64, 128],
            [256, 512, 1024, 2048],
            [4096, 8192, 16384, 32768],
        ])
        .unwrap();

        let roundtrip = Grid::from_rows(grid.rows());

        assert_eq!(roundtrip, grid);
    }

    #[test]
    fn can_make_grid_from_columns() {
        let col0 = Column::from_row(Row::pack([0, 4, 8, 12]).unwrap());
        let col1 = Column::from_row(Row::pack([1, 5, 9, 13]).unwrap());
        let col2 = Column::from_row(Row::pack([2, 6, 10, 14]).unwrap());
        let col3 = Column::from_row(Row::pack([3, 7, 11, 15]).unwrap());
        let grid = Grid::from_columns([col0, col1, col2, col3]);
        let expected =
            Grid::from_log([[0, 1, 2, 3], [4, 5, 6, 7], [8, 9, 10, 11], [12, 13, 14, 15]]).unwrap();

        assert_eq!(grid, expected);
    }
}
