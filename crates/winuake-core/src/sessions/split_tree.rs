// Ported from yakuake app/session.cpp and app/splitter.cpp @21c6210
//! The layout of terminals inside one session: a binary tree of splitters. Yakuake keeps Qt splitters with pixel sizes; this
//! keeps a ratio per splitter and derives pixel rectangles on demand, so the logic runs (and is tested) without a UI.

use crate::ids::TerminalId;

/// Direction a splitter lays its two children out. `Horizontal` puts them side by side (Qt::Horizontal), `Vertical` stacks them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Horizontal,
    Vertical,
}

/// Where a terminal asks to grow. Ported from `Session::GrowthDirection`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrowthDirection {
    Up,
    Right,
    Down,
    Left,
}

impl GrowthDirection {
    fn orientation(self) -> Orientation {
        match self {
            Self::Left | Self::Right => Orientation::Horizontal,
            Self::Up | Self::Down => Orientation::Vertical,
        }
    }

    /// Toward the second child of a splitter.
    fn is_forward(self) -> bool {
        matches!(self, Self::Right | Self::Down)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaneRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl PaneRect {
    pub const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self { x, y, width, height }
    }

    fn extent(&self, orientation: Orientation) -> i32 {
        match orientation {
            Orientation::Horizontal => self.width,
            Orientation::Vertical => self.height,
        }
    }

    /// Splits the rectangle along `orientation` into the first pane, the divider, and the second pane.
    fn divide(&self, orientation: Orientation, first_extent: i32, divider: i32) -> [PaneRect; 3] {
        match orientation {
            Orientation::Horizontal => {
                let second_x = self.x + first_extent + divider;
                [
                    PaneRect::new(self.x, self.y, first_extent, self.height),
                    PaneRect::new(self.x + first_extent, self.y, divider, self.height),
                    PaneRect::new(second_x, self.y, self.width - first_extent - divider, self.height),
                ]
            }
            Orientation::Vertical => {
                let second_y = self.y + first_extent + divider;
                [
                    PaneRect::new(self.x, self.y, self.width, first_extent),
                    PaneRect::new(self.x, self.y + first_extent, self.width, divider),
                    PaneRect::new(self.x, second_y, self.width, self.height - first_extent - divider),
                ]
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Branch {
    First,
    Second,
}

/// Route from the root to one splitter.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SplitPath(pub Vec<Branch>);

#[derive(Debug, Clone, PartialEq)]
pub struct PaneLayout {
    pub terminal: TerminalId,
    pub rect: PaneRect,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DividerLayout {
    pub path: SplitPath,
    pub orientation: Orientation,
    pub rect: PaneRect,
    /// Whole area of the splitter that owns this divider; a drag position is a fraction of it.
    pub owner_area: PaneRect,
}

#[derive(Debug, Clone, PartialEq)]
enum Node {
    Leaf(TerminalId),
    Split(Box<Split>),
}

#[derive(Debug, Clone, PartialEq)]
struct Split {
    orientation: Orientation,
    first: Node,
    second: Node,
    /// Share of the available extent given to `first`, 0..1.
    ratio: f32,
}

impl Node {
    fn contains(&self, target: TerminalId) -> bool {
        match self {
            Node::Leaf(id) => *id == target,
            Node::Split(split) => split.first.contains(target) || split.second.contains(target),
        }
    }

    fn collect_leaves(&self, into: &mut Vec<TerminalId>) {
        match self {
            Node::Leaf(id) => into.push(*id),
            Node::Split(split) => {
                split.first.collect_leaves(into);
                split.second.collect_leaves(into);
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SplitTree {
    root: Node,
}

/// Smallest share a pane keeps when a divider is moved.
const MINIMUM_RATIO: f32 = 0.05;

impl SplitTree {
    pub fn single(terminal: TerminalId) -> Self {
        Self { root: Node::Leaf(terminal) }
    }

    /// Two terminals in one splitter, half each.
    pub fn pair(orientation: Orientation, first: TerminalId, second: TerminalId) -> Self {
        Self { root: Self::split_node(orientation, Node::Leaf(first), Node::Leaf(second)) }
    }

    /// 2x2 grid: a vertical splitter of two horizontal rows (Yakuake `Session::Quad`).
    pub fn grid(top: [TerminalId; 2], bottom: [TerminalId; 2]) -> Self {
        let upper = Self::split_node(Orientation::Horizontal, Node::Leaf(top[0]), Node::Leaf(top[1]));
        let lower = Self::split_node(Orientation::Horizontal, Node::Leaf(bottom[0]), Node::Leaf(bottom[1]));
        Self { root: Self::split_node(Orientation::Vertical, upper, lower) }
    }

    fn split_node(orientation: Orientation, first: Node, second: Node) -> Node {
        Node::Split(Box::new(Split { orientation, first, second, ratio: 0.5 }))
    }

    pub fn contains(&self, terminal: TerminalId) -> bool {
        self.root.contains(terminal)
    }

    /// Terminals in layout order (top-left first).
    pub fn terminals_in_layout_order(&self) -> Vec<TerminalId> {
        let mut leaves = Vec::new();
        self.root.collect_leaves(&mut leaves);
        leaves
    }

    /// Replaces `target` with a splitter holding it and `added`, 50/50. False when `target` is not in the tree.
    pub fn split(&mut self, target: TerminalId, orientation: Orientation, added: TerminalId) -> bool {
        Self::split_in(&mut self.root, target, orientation, added)
    }

    fn split_in(node: &mut Node, target: TerminalId, orientation: Orientation, added: TerminalId) -> bool {
        match node {
            Node::Leaf(id) if *id == target => {
                *node = Self::split_node(orientation, Node::Leaf(target), Node::Leaf(added));
                true
            }
            Node::Leaf(_) => false,
            Node::Split(split) => {
                Self::split_in(&mut split.first, target, orientation, added)
                    || Self::split_in(&mut split.second, target, orientation, added)
            }
        }
    }

    /// Removes `target`; its sibling takes the whole splitter. False when it is absent or the only terminal.
    pub fn remove(&mut self, target: TerminalId) -> bool {
        match &mut self.root {
            Node::Leaf(_) => false,
            root => Self::remove_in(root, target),
        }
    }

    fn remove_in(node: &mut Node, target: TerminalId) -> bool {
        let Node::Split(split) = node else { return false };
        let replacement = match (&split.first, &split.second) {
            (Node::Leaf(id), _) if *id == target => Some(split.second.clone()),
            (_, Node::Leaf(id)) if *id == target => Some(split.first.clone()),
            _ => None,
        };
        if let Some(replacement) = replacement {
            *node = replacement;
            return true;
        }
        Self::remove_in(&mut split.first, target) || Self::remove_in(&mut split.second, target)
    }

    /// Pixel rectangles of every terminal inside `area`, with `divider` pixels between panes.
    pub fn layout(&self, area: PaneRect, divider: i32) -> Vec<PaneLayout> {
        let mut panes = Vec::new();
        Self::layout_in(&self.root, area, divider, &mut panes, &mut Vec::new(), &mut SplitPath::default());
        panes
    }

    /// Divider handles, one per splitter.
    pub fn dividers(&self, area: PaneRect, divider: i32) -> Vec<DividerLayout> {
        let mut handles = Vec::new();
        Self::layout_in(&self.root, area, divider, &mut Vec::new(), &mut handles, &mut SplitPath::default());
        handles
    }

    fn layout_in(
        node: &Node,
        area: PaneRect,
        divider: i32,
        panes: &mut Vec<PaneLayout>,
        handles: &mut Vec<DividerLayout>,
        path: &mut SplitPath,
    ) {
        match node {
            Node::Leaf(terminal) => panes.push(PaneLayout { terminal: *terminal, rect: area }),
            Node::Split(split) => {
                let [first_area, divider_area, second_area] = split.areas(area, divider);
                handles.push(DividerLayout {
                    path: path.clone(),
                    orientation: split.orientation,
                    rect: divider_area,
                    owner_area: area,
                });
                path.0.push(Branch::First);
                Self::layout_in(&split.first, first_area, divider, panes, handles, path);
                path.0.pop();
                path.0.push(Branch::Second);
                Self::layout_in(&split.second, second_area, divider, panes, handles, path);
                path.0.pop();
            }
        }
    }

    /// Moves a divider so the first pane takes `ratio` of the splitter, kept inside the minimum share.
    pub fn set_ratio(&mut self, path: &SplitPath, ratio: f32) -> bool {
        let mut node = &mut self.root;
        for branch in &path.0 {
            let Node::Split(split) = node else { return false };
            node = match branch {
                Branch::First => &mut split.first,
                Branch::Second => &mut split.second,
            };
        }
        let Node::Split(split) = node else { return false };
        split.ratio = ratio.clamp(MINIMUM_RATIO, 1.0 - MINIMUM_RATIO);
        true
    }

    /// Ported from `Session::tryGrowTerminal`. Moves the divider next to `terminal` toward its neighbor by up to `pixels`
    /// and returns how much the terminal grew. `None` when no splitter lets this terminal grow that way.
    pub fn try_grow(
        &mut self,
        terminal: TerminalId,
        direction: GrowthDirection,
        pixels: i32,
        area: PaneRect,
        divider: i32,
        minimum_extent: i32,
    ) -> Option<i32> {
        let attempt = Self::grow_in(&mut self.root, terminal, direction, pixels, area, divider, minimum_extent);
        match attempt {
            GrowAttempt::Moved(grown) => Some(grown),
            GrowAttempt::NotHere | GrowAttempt::NotFound => None,
        }
    }

    fn grow_in(
        node: &mut Node,
        terminal: TerminalId,
        direction: GrowthDirection,
        pixels: i32,
        area: PaneRect,
        divider: i32,
        minimum_extent: i32,
    ) -> GrowAttempt {
        let Node::Split(split) = node else { return GrowAttempt::NotFound };
        let in_first = split.first.contains(terminal);
        if !in_first && !split.second.contains(terminal) {
            return GrowAttempt::NotFound;
        }
        let [first_area, _, second_area] = split.areas(area, divider);
        let (child, child_area) =
            if in_first { (&mut split.first, first_area) } else { (&mut split.second, second_area) };
        let attempt = Self::grow_in(child, terminal, direction, pixels, child_area, divider, minimum_extent);
        if matches!(attempt, GrowAttempt::Moved(_)) {
            return attempt;
        }
        // Not movable lower down: this splitter can move its divider if the terminal sits on the side that grows.
        let faces_neighbor = direction.is_forward() == in_first;
        if split.orientation != direction.orientation() || !faces_neighbor {
            return GrowAttempt::NotHere;
        }
        let available = area.extent(split.orientation) - divider;
        if available <= 0 {
            return GrowAttempt::NotHere;
        }
        let first_extent = (available as f32 * split.ratio).round() as i32;
        let second_extent = available - first_extent;
        let (own, neighbor) = if in_first { (first_extent, second_extent) } else { (second_extent, first_extent) };
        let grown = pixels.clamp(0, (neighbor - minimum_extent).max(0));
        let new_first = if in_first { own + grown } else { neighbor - grown };
        split.ratio = new_first as f32 / available as f32;
        GrowAttempt::Moved(grown)
    }
}

enum GrowAttempt {
    Moved(i32),
    NotHere,
    NotFound,
}

impl Split {
    fn areas(&self, area: PaneRect, divider: i32) -> [PaneRect; 3] {
        let available = (area.extent(self.orientation) - divider).max(0);
        let first_extent = (available as f32 * self.ratio).round() as i32;
        area.divide(self.orientation, first_extent, divider)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AREA: PaneRect = PaneRect::new(0, 0, 1001, 600);
    const A: TerminalId = TerminalId(1);
    const B: TerminalId = TerminalId(2);
    const C: TerminalId = TerminalId(3);
    const D: TerminalId = TerminalId(4);

    fn rect_of(tree: &SplitTree, terminal: TerminalId) -> PaneRect {
        tree.layout(AREA, 1).into_iter().find(|pane| pane.terminal == terminal).expect("terminal in layout").rect
    }

    #[test]
    fn given_a_single_terminal_when_laid_out_then_it_fills_the_area() {
        assert_eq!(rect_of(&SplitTree::single(A), A), AREA);
    }

    #[test]
    fn given_a_left_right_split_when_laid_out_then_panes_sit_side_by_side_around_the_divider() {
        let mut tree = SplitTree::single(A);
        assert!(tree.split(A, Orientation::Horizontal, B));
        assert_eq!(rect_of(&tree, A), PaneRect::new(0, 0, 500, 600));
        assert_eq!(rect_of(&tree, B), PaneRect::new(501, 0, 500, 600));
    }

    #[test]
    fn given_a_top_bottom_split_when_laid_out_then_the_new_terminal_is_below() {
        let mut tree = SplitTree::single(A);
        tree.split(A, Orientation::Vertical, B);
        assert_eq!(rect_of(&tree, A), PaneRect::new(0, 0, 1001, 300));
        assert_eq!(rect_of(&tree, B).y, 300 + 1);
    }

    #[test]
    fn given_a_split_of_one_half_when_splitting_again_then_only_that_half_is_divided() {
        let mut tree = SplitTree::pair(Orientation::Horizontal, A, B);
        tree.split(B, Orientation::Vertical, C);
        assert_eq!(rect_of(&tree, A), PaneRect::new(0, 0, 500, 600));
        assert_eq!(rect_of(&tree, B).x, 501);
        assert_eq!(rect_of(&tree, B).height, 300);
        assert_eq!(rect_of(&tree, C).y, 301);
        assert_eq!(tree.terminals_in_layout_order(), vec![A, B, C]);
    }

    #[test]
    fn given_an_unknown_terminal_when_splitting_then_nothing_changes() {
        let mut tree = SplitTree::single(A);
        assert!(!tree.split(B, Orientation::Horizontal, C));
        assert_eq!(tree.terminals_in_layout_order(), vec![A]);
    }

    #[test]
    fn given_a_grid_when_laid_out_then_four_equal_quadrants_appear() {
        let tree = SplitTree::grid([A, B], [C, D]);
        assert_eq!(tree.terminals_in_layout_order(), vec![A, B, C, D]);
        assert_eq!(rect_of(&tree, A), PaneRect::new(0, 0, 500, 300));
        assert_eq!(rect_of(&tree, D), PaneRect::new(501, 300 + 1, 500, 299));
    }

    #[test]
    fn given_two_terminals_when_one_is_removed_then_the_other_fills_the_area() {
        let mut tree = SplitTree::pair(Orientation::Horizontal, A, B);
        assert!(tree.remove(A));
        assert_eq!(rect_of(&tree, B), AREA);
        assert!(!tree.contains(A));
    }

    #[test]
    fn given_a_nested_split_when_the_inner_terminal_is_removed_then_its_sibling_takes_the_slot() {
        let mut tree = SplitTree::pair(Orientation::Horizontal, A, B);
        tree.split(B, Orientation::Vertical, C);
        assert!(tree.remove(C));
        assert_eq!(rect_of(&tree, B), PaneRect::new(501, 0, 500, 600));
    }

    #[test]
    fn given_the_only_terminal_when_removed_then_it_stays() {
        let mut tree = SplitTree::single(A);
        assert!(!tree.remove(A));
    }

    #[test]
    fn given_a_missing_terminal_when_removing_then_false() {
        let mut tree = SplitTree::pair(Orientation::Horizontal, A, B);
        assert!(!tree.remove(C));
    }

    #[test]
    fn given_a_divider_drag_when_setting_ratio_then_the_panes_resize_and_stay_above_the_minimum() {
        let mut tree = SplitTree::pair(Orientation::Horizontal, A, B);
        let path = tree.dividers(AREA, 1)[0].path.clone();
        assert!(tree.set_ratio(&path, 0.25));
        assert_eq!(rect_of(&tree, A).width, 250);
        assert!(tree.set_ratio(&path, 0.0));
        assert_eq!(rect_of(&tree, A).width, 50);
    }

    #[test]
    fn given_a_bad_path_when_setting_ratio_then_false() {
        let mut tree = SplitTree::single(A);
        assert!(!tree.set_ratio(&SplitPath::default(), 0.3));
        let mut pair = SplitTree::pair(Orientation::Horizontal, A, B);
        assert!(!pair.set_ratio(&SplitPath(vec![Branch::First]), 0.3));
        assert!(!pair.set_ratio(&SplitPath(vec![Branch::First, Branch::Second]), 0.3));
    }

    #[test]
    fn given_a_grid_when_asking_for_dividers_then_there_is_one_per_splitter_with_its_owner_area() {
        let tree = SplitTree::grid([A, B], [C, D]);
        let dividers = tree.dividers(AREA, 1);
        assert_eq!(dividers.len(), 3);
        assert_eq!(dividers[0].orientation, Orientation::Vertical);
        assert_eq!(dividers[0].owner_area, AREA);
        assert_eq!(dividers[1].path, SplitPath(vec![Branch::First]));
    }

    #[test]
    fn given_the_left_terminal_when_growing_right_then_the_divider_moves_and_the_growth_is_reported() {
        let mut tree = SplitTree::pair(Orientation::Horizontal, A, B);
        assert_eq!(tree.try_grow(A, GrowthDirection::Right, 100, AREA, 1, 20), Some(100));
        assert_eq!(rect_of(&tree, A).width, 600);
    }

    #[test]
    fn given_the_right_terminal_when_growing_left_then_it_widens_at_the_neighbors_expense() {
        let mut tree = SplitTree::pair(Orientation::Horizontal, A, B);
        assert_eq!(tree.try_grow(B, GrowthDirection::Left, 100, AREA, 1, 20), Some(100));
        assert_eq!(rect_of(&tree, B).width, 600);
        assert_eq!(rect_of(&tree, A).width, 400);
    }

    #[test]
    fn given_the_left_terminal_when_growing_left_then_nothing_can_move() {
        let mut tree = SplitTree::pair(Orientation::Horizontal, A, B);
        assert_eq!(tree.try_grow(A, GrowthDirection::Left, 100, AREA, 1, 20), None);
        assert_eq!(tree.try_grow(A, GrowthDirection::Up, 100, AREA, 1, 20), None);
    }

    #[test]
    fn given_a_nested_terminal_when_growing_across_the_outer_splitter_then_the_outer_divider_moves() {
        let mut tree = SplitTree::pair(Orientation::Horizontal, A, B);
        tree.split(B, Orientation::Vertical, C);
        assert_eq!(tree.try_grow(C, GrowthDirection::Left, 50, AREA, 1, 20), Some(50));
        assert_eq!(rect_of(&tree, A).width, 450);
        assert_eq!(rect_of(&tree, C).width, 550);
    }

    #[test]
    fn given_a_growth_beyond_the_neighbor_when_growing_then_it_stops_at_the_minimum_extent() {
        let mut tree = SplitTree::pair(Orientation::Horizontal, A, B);
        assert_eq!(tree.try_grow(A, GrowthDirection::Right, 10_000, AREA, 1, 20), Some(480));
        assert_eq!(rect_of(&tree, B).width, 20);
    }

    #[test]
    fn given_an_unknown_terminal_when_growing_then_none() {
        let mut tree = SplitTree::pair(Orientation::Horizontal, A, B);
        assert_eq!(tree.try_grow(C, GrowthDirection::Right, 10, AREA, 1, 20), None);
    }
}
