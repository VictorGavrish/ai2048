//! This module intends to provide a lazily-evaluated, cached tree of all possible board states
//! in a 2048 game.
//!
//! The types in this module generate its children only once.
//!
//! They use two different kinds of cache to reduce the amount of computation as much as possible:
//!
//! 1. Each node stores references to its children.
//! 2. When generating the children, the nodes query a `Cache` of known nodes (a transposition
//! table) in case this same node has already been generated through a different set of moves.
//!
//! It achieves this by a combination of interior mutability, reference counted objects and
//! a hashmap.

mod cache;

use crate::board::{self, Board, Move};
use crate::search_tree::cache::Cache;
use lazycell::LazyCell;
use std::cell::Cell;
use std::rc::Rc;

struct NodeCache<T>
where
    T: Copy + Default,
{
    player_node: Cache<Board, PlayerNode<T>>,
    computer_node: Cache<Board, ComputerNode<T>>,
}

/// The `SearchTree` type is the root of the tree of nodes that form all possible board states in
/// a 2048 game. It is the only potentially mutable type in this module. You can generate a new
/// `SearchTree` by providing an initial board state, or use a mutable reference to an existing
/// `SearchTree` to update its root board state in order to reuse nodes already calculated from
/// the previous state.
pub struct SearchTree<T>
where
    T: Copy + Default,
{
    root_node: Rc<PlayerNode<T>>,
    cache: Rc<NodeCache<T>>,
}

impl<T> SearchTree<T>
where
    T: Copy + Default,
{
    /// Creates a new `SearchTree` from an initial `Board` state.
    pub fn new(board: Board) -> Self {
        let cache = Rc::new(NodeCache {
            player_node: Cache::new(),
            computer_node: Cache::new(),
        });

        let node = cache
            .player_node
            .get_or_insert_with(board, || PlayerNode::new(board, cache.clone()));

        SearchTree {
            root_node: node,
            cache,
        }
    }

    /// Updates the search tree to have a different root `Board` state. It has an advantage over
    /// creating a new one because it reuses the inner cache of known nodes. This implicitly
    /// invalidates now unreachable board states in the cache (or at least board states that
    /// have no known way to be reached). This also explicitly cleans up the invalidated keys
    /// from the cache.
    pub fn set_root(&mut self, board: Board) {
        let node = self
            .cache
            .player_node
            .get_or_insert_with(board, || PlayerNode::new(board, self.cache.clone()));

        self.root_node = node;

        self.clean_up_cache();
    }

    /// Gets a reference to the current root node.
    pub fn root(&self) -> &PlayerNode<T> {
        self.root_node.as_ref()
    }

    /// Gets the number of known board states that the Player can face on their turn.
    pub fn known_player_node_count(&self) -> usize {
        self.cache.player_node.strong_count()
    }

    /// Gets the number of known board states that the Computer can face on its turn.
    pub fn known_computer_node_count(&self) -> usize {
        self.cache.computer_node.strong_count()
    }

    fn clean_up_cache(&self) {
        self.cache.player_node.gc();
        self.cache.computer_node.gc();
    }
}

/// This type represents the children of a `PlayerNode`.
pub struct PlayerNodeChildren<T>
where
    T: Copy + Default,
{
    nodes: [Option<Rc<ComputerNode<T>>>; 4],
}

impl<T> PlayerNodeChildren<T>
where
    T: Copy + Default,
{
    /// Returns true if there are no children. This is true for a game over node's children.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.nodes.iter().all(|n| n.is_none())
    }

    /// Iterates over children, returning `(Move, &ComputerNode)` tuples.
    #[inline]
    pub fn iter<'a>(&'a self) -> impl Iterator<Item = (Move, &'a ComputerNode<T>)> + 'a {
        self.nodes.iter().enumerate().filter_map(|(index, opt)| {
            opt.as_ref().map(|node| {
                let mv = match index {
                    0 => Move::Left,
                    1 => Move::Right,
                    2 => Move::Up,
                    3 => Move::Down,
                    _ => unreachable!(),
                };

                (mv, node.as_ref())
            })
        })
    }

    /// Iterates over children, returning `&ComputerNode`s without moves.
    #[inline]
    pub fn values<'a>(&'a self) -> impl Iterator<Item = &'a ComputerNode<T>> + 'a {
        self.nodes
            .iter()
            .filter_map(|opt| opt.as_ref().map(|node| node.as_ref()))
    }
}

/// This type represents a `Board` state that can be reached on the Player's turn. This type
/// is logically immutable, and there should be no way to create this type from outside the module
/// through any means other than querying the `SearchTree` root and its descendants.
///
/// However, this type makes use of interior mutability to defer generating its children until
/// such time as it is asked to do so, and only do it once even then.
pub struct PlayerNode<T>
where
    T: Copy + Default,
{
    board: Board,
    cache: Rc<NodeCache<T>>,
    children: LazyCell<PlayerNodeChildren<T>>,
    pub data: Cell<T>,
}

impl<T> PlayerNode<T>
where
    T: Copy + Default,
{
    fn new(board: Board, cache: Rc<NodeCache<T>>) -> Self {
        PlayerNode {
            board,
            cache,
            children: LazyCell::new(),
            data: Cell::new(T::default()),
        }
    }

    /// Get a reference to the `Board` state associated with this node.
    pub fn board(&self) -> &Board {
        &self.board
    }

    /// Returns a `PlayerNodeChildren` which represents all possible `Move`:`ComputerNode` pairs
    /// possible in the current position.
    pub fn children(&self) -> &PlayerNodeChildren<T> {
        self.children.borrow_with(|| self.create_children())
    }

    fn create_children(&self) -> PlayerNodeChildren<T> {
        let mut children = [None, None, None, None];

        for &m in &board::MOVES {
            let new_grid = self.board.make_move(m);

            // It is illegal to make a move that doesn't change anything.
            if new_grid != self.board {
                let computer_node = self.cache.computer_node.get_or_insert_with(new_grid, || {
                    ComputerNode::new(new_grid, self.cache.clone())
                });

                children[m as u8 as usize] = Some(computer_node);
            }
        }

        PlayerNodeChildren { nodes: children }
    }
}

/// This type holds all the children of a computer node. It is useful to separate the children
/// that were generated by spawning a 2 from ones that were spawned with a 4, because in a game
/// of 2048 a 4 only spawns 10% of the time, and it's important to take into account how likely
/// an outcome is.
pub struct ComputerNodeChildren<T>
where
    T: Copy + Default,
{
    with2: Vec<Rc<PlayerNode<T>>>,
    with4: Vec<Rc<PlayerNode<T>>>,
}

impl<T> ComputerNodeChildren<T>
where
    T: Copy + Default,
{
    /// Game states generated by the computer spawning a 2.
    #[inline]
    pub fn with2<'a>(&'a self) -> impl Iterator<Item = &'a PlayerNode<T>> + 'a {
        self.with2.iter().map(|n| n.as_ref())
    }

    /// Game states generated by the computer spawning a 4.
    #[inline]
    pub fn with4<'a>(&'a self) -> impl Iterator<Item = &'a PlayerNode<T>> + 'a {
        self.with4.iter().map(|n| n.as_ref())
    }

    /// Number of variants of either children
    pub fn variants(&self) -> usize {
        self.with2.len()
    }
}

/// This type represents a `Board` state that can be reached on the Computer's turn. This type
/// is logically immutable, and there should be no way to create this type from outside the module
/// through any means other than querying a `PlayerNode`.
///
/// However, this type makes use of interior mutability to defer generating its children until
/// such time as it is asked to do so, and only do it once even then.
pub struct ComputerNode<T>
where
    T: Copy + Default,
{
    board: Board,
    cache: Rc<NodeCache<T>>,
    children: LazyCell<ComputerNodeChildren<T>>,
}

impl<T> ComputerNode<T>
where
    T: Copy + Default,
{
    fn new(board: Board, cache: Rc<NodeCache<T>>) -> Self {
        ComputerNode {
            board,
            cache,
            children: LazyCell::new(),
        }
    }

    /// Get a reference to the `Board` state associated with this node.
    pub fn board(&self) -> &Board {
        &self.board
    }

    /// Returns an `ComputerNodeChildren` that represents all possible states that the Player
    /// can face following a computer spawning a random 2 or 4 tile. Can't be empty, by the game'search_tree
    /// logic.
    pub fn children(&self) -> &ComputerNodeChildren<T> {
        self.children.borrow_with(|| self.create_children())
    }

    fn create_children(&self) -> ComputerNodeChildren<T> {
        let children_with2 = self
            .board
            .ai_moves_with2()
            .into_iter()
            .map(|board| {
                self.cache
                    .player_node
                    .get_or_insert_with(board, || PlayerNode::new(board, self.cache.clone()))
            })
            .collect::<Vec<_>>();

        let children_with4 = self
            .board
            .ai_moves_with4()
            .into_iter()
            .map(|board| {
                self.cache
                    .player_node
                    .get_or_insert_with(board, || PlayerNode::new(board, self.cache.clone()))
            })
            .collect::<Vec<_>>();

        debug_assert!(!children_with2.is_empty());
        debug_assert!(!children_with4.is_empty());

        ComputerNodeChildren {
            with2: children_with2,
            with4: children_with4,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{Board, Move};
    use std::collections::{HashMap, HashSet};

    #[test]
    fn can_create_new_search_tree() {
        let expected_grid = Board::default().add_random_tile();
        let search_tree: SearchTree<()> = SearchTree::new(expected_grid);
        let actual_grid = *search_tree.root().board();

        assert_eq!(expected_grid, actual_grid);
    }

    #[test]
    fn can_set_new_root() {
        let grid1 = Board::default().add_random_tile();
        let grid2 = Board::default().add_random_tile().add_random_tile();
        let mut search_tree: SearchTree<()> = SearchTree::new(grid1);

        search_tree.set_root(grid2);

        assert_eq!(grid2, *search_tree.root().board());
        assert_eq!(1, search_tree.known_player_node_count());
        let total = search_tree.cache.player_node.len();
        assert_eq!(1, total);
    }

    #[test]
    #[cfg_attr(rustfmt, rustfmt_skip)]
    fn can_player_node_children_by_move() {
        let board = Board::from_u32([
            [0, 0, 0, 2],
            [0, 2, 0, 2],
            [4, 0, 0, 2],
            [0, 0, 0, 2],
        ]).unwrap();

        let search_tree: SearchTree<()> = SearchTree::new(board);

        let player_node = search_tree.root();

        let mut expected = HashMap::new();
        expected.insert(Move::Left, Board::from_u32([
            [2, 0, 0, 0],
            [4, 0, 0, 0],
            [4, 2, 0, 0],
            [2, 0, 0, 0],
        ]).unwrap());
        expected.insert(Move::Right, Board::from_u32([
            [0, 0, 0, 2],
            [0, 0, 0, 4],
            [0, 0, 4, 2],
            [0, 0, 0, 2],
        ]).unwrap());
        expected.insert(Move::Up, Board::from_u32([
            [4, 2, 0, 4],
            [0, 0, 0, 4],
            [0, 0, 0, 0],
            [0, 0, 0, 0],
        ]).unwrap());
        expected.insert(Move::Down, Board::from_u32([
            [0, 0, 0, 0],
            [0, 0, 0, 0],
            [0, 0, 0, 4],
            [4, 2, 0, 4],
        ]).unwrap());

        let actual = player_node.children().iter().collect::<HashMap<_, _>>();

        for (key, value) in expected {
            assert_eq!(value, *actual.get(&key).unwrap().board());
        }

        assert_eq!(1, search_tree.known_player_node_count());
        assert_eq!(4, search_tree.known_computer_node_count());
    }

    #[test]
    #[cfg_attr(rustfmt, rustfmt_skip)]
    fn can_computer_node_children() {
        let board = Board::from_u32([
            [0, 2, 4, 2],
            [0, 4, 2, 4],
            [4, 2, 4, 2],
            [2, 4, 2, 4],
        ]).unwrap();
        let search_tree: SearchTree<()> = SearchTree::new(board);

        // two possible moves: up and left
        // up:   [4, 2, 4, 2],
        //       [2, 4, 2, 4],
        //       [0, 2, 4, 2],
        //       [0, 4, 2, 4]
        //
        // left: [2, 4, 2, 0],
        //       [4, 2, 4, 0],
        //       [4, 2, 4, 2],
        //       [2, 4, 2, 4]

        // this leads to 8 possible child nodes:
        let mut expected_with2 = HashSet::new();
        expected_with2.insert(Board::from_u32([
            [4, 2, 4, 2],
            [2, 4, 2, 4],
            [2, 2, 4, 2],
            [0, 4, 2, 4],
        ]).unwrap());
        expected_with2.insert(Board::from_u32([
            [4, 2, 4, 2],
            [2, 4, 2, 4],
            [0, 2, 4, 2],
            [2, 4, 2, 4],
        ]).unwrap());
        expected_with2.insert(Board::from_u32([
            [2, 4, 2, 2],
            [4, 2, 4, 0],
            [4, 2, 4, 2],
            [2, 4, 2, 4],
        ]).unwrap());
        expected_with2.insert(Board::from_u32([
            [2, 4, 2, 0],
            [4, 2, 4, 2],
            [4, 2, 4, 2],
            [2, 4, 2, 4],
        ]).unwrap());

        let mut expected_with4 = HashSet::new();
        expected_with4.insert(Board::from_u32([
            [2, 4, 2, 4],
            [4, 2, 4, 0],
            [4, 2, 4, 2],
            [2, 4, 2, 4],
        ]).unwrap());
        expected_with4.insert(Board::from_u32([
            [2, 4, 2, 0],
            [4, 2, 4, 4],
            [4, 2, 4, 2],
            [2, 4, 2, 4],
        ]).unwrap());
        expected_with4.insert(Board::from_u32([
            [4, 2, 4, 2],
            [2, 4, 2, 4],
            [4, 2, 4, 2],
            [0, 4, 2, 4],
        ]).unwrap());
        expected_with4.insert(Board::from_u32([
            [4, 2, 4, 2],
            [2, 4, 2, 4],
            [0, 2, 4, 2],
            [4, 4, 2, 4],
        ]).unwrap());

        let actual_with2 = search_tree.root()
            .children()
            .values()
            .flat_map(|v| v.children().with2())
            .map(|n| *n.board())
            .collect::<HashSet<_>>();

        let actual_with4 = search_tree.root()
            .children()
            .values()
            .flat_map(|v| v.children().with4())
            .map(|n| *n.board())
            .collect::<HashSet<_>>();

        assert_eq!(expected_with2, actual_with2);
        assert_eq!(expected_with4, actual_with4);
    }
}