//! Sockets: the one connection rule behind walls, doors and rooms.
//!
//! Every wall glyph exposes a socket on each edge (north, east, south, west):
//! 0 none, 1 single stroke, 2 double. A socket must land on a wall or a door.
//! A door (`+`) sits in a wall line and links the two rooms either side of it.
//!
//! From that rule:
//! - [`check`] finds broken walls, stray doors, rooms open to the outside and
//!   rooms with no door path to the entrance;
//! - [`fix`] bridges small wall gaps with a door, redraws every wall from its
//!   neighbours, and cuts a door into each sealed room;
//! - the builder ([`crate::builder`]) drops pieces at the cursor and lets `fix` finish them.

use std::collections::VecDeque;
use std::fmt;

/// The door glyph.
pub const DOOR: char = '+';

/// Widest wall gap [`fix`] bridges with a door.
pub const MAX_GAP: usize = 4;

const N: usize = 0;
const E: usize = 1;
const S: usize = 2;
const W: usize = 3;
const STEP: [(isize, isize); 4] = [(-1, 0), (0, 1), (1, 0), (0, -1)];
const OPP: [usize; 4] = [S, W, N, E];
const SIDE: [char; 4] = ['N', 'E', 'S', 'W'];

/// The 44 box-drawing glyphs and their sockets `[n, e, s, w]`.
pub const WALLS: [(char, [u8; 4]); 44] = [
    ('─', [0, 1, 0, 1]),
    ('│', [1, 0, 1, 0]),
    ('┌', [0, 1, 1, 0]),
    ('┐', [0, 0, 1, 1]),
    ('└', [1, 1, 0, 0]),
    ('┘', [1, 0, 0, 1]),
    ('├', [1, 1, 1, 0]),
    ('┤', [1, 0, 1, 1]),
    ('┬', [0, 1, 1, 1]),
    ('┴', [1, 1, 0, 1]),
    ('┼', [1, 1, 1, 1]),
    ('═', [0, 2, 0, 2]),
    ('║', [2, 0, 2, 0]),
    ('╔', [0, 2, 2, 0]),
    ('╗', [0, 0, 2, 2]),
    ('╚', [2, 2, 0, 0]),
    ('╝', [2, 0, 0, 2]),
    ('╠', [2, 2, 2, 0]),
    ('╣', [2, 0, 2, 2]),
    ('╦', [0, 2, 2, 2]),
    ('╩', [2, 2, 0, 2]),
    ('╬', [2, 2, 2, 2]),
    ('╒', [0, 2, 1, 0]),
    ('╓', [0, 1, 2, 0]),
    ('╕', [0, 0, 1, 2]),
    ('╖', [0, 0, 2, 1]),
    ('╘', [1, 2, 0, 0]),
    ('╙', [2, 1, 0, 0]),
    ('╛', [1, 0, 0, 2]),
    ('╜', [2, 0, 0, 1]),
    ('╞', [1, 2, 1, 0]),
    ('╟', [2, 1, 2, 0]),
    ('╡', [1, 0, 1, 2]),
    ('╢', [2, 0, 2, 1]),
    ('╤', [0, 2, 1, 2]),
    ('╥', [0, 1, 2, 1]),
    ('╧', [1, 2, 0, 2]),
    ('╨', [2, 1, 0, 1]),
    ('╪', [1, 2, 1, 2]),
    ('╫', [2, 1, 2, 1]),
    ('╵', [1, 0, 0, 0]),
    ('╶', [0, 1, 0, 0]),
    ('╷', [0, 0, 1, 0]),
    ('╴', [0, 0, 0, 1]),
];

/// Sockets of a wall glyph, or `None` if it is not a wall.
pub fn wall_sockets(ch: char) -> Option<[u8; 4]> {
    WALLS.iter().find(|w| w.0 == ch).map(|w| w.1)
}

/// The wall glyph with exactly these sockets, if one exists.
pub fn wall_glyph(sockets: [u8; 4]) -> Option<char> {
    WALLS.iter().find(|w| w.1 == sockets).map(|w| w.0)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Wall,
    Door,
    Open,
}

fn kind_of(ch: char) -> Kind {
    if ch == DOOR {
        Kind::Door
    } else if wall_sockets(ch).is_some() {
        Kind::Wall
    } else {
        Kind::Open
    }
}

/// An editable plan: rows of characters, padded to a rectangle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grid {
    cells: Vec<Vec<char>>,
    cols: usize,
}

impl Grid {
    /// Parse plan text (a `.ptex` `\topology{}` body is honoured), clipped to 71 x 48.
    pub fn parse(src: &str) -> Self {
        let mut cells: Vec<Vec<char>> = crate::body_lines(src)
            .iter()
            .take(crate::MAX_ROWS)
            .map(|l| l.chars().take(crate::MAX_COLS).collect())
            .collect();
        let cols = cells.iter().map(Vec::len).max().unwrap_or(0);
        for row in &mut cells {
            row.resize(cols, ' ');
        }
        Self { cells, cols }
    }

    /// An all-space grid.
    pub fn blank(cols: usize, rows: usize) -> Self {
        Self {
            cells: vec![vec![' '; cols]; rows],
            cols,
        }
    }

    /// Pad with spaces to at least `cols x rows` (clamped to 71 x 48). Never shrinks.
    pub fn grow(&mut self, cols: usize, rows: usize) {
        self.cols = self.cols.max(cols.min(crate::MAX_COLS));
        let rows = self.cells.len().max(rows.min(crate::MAX_ROWS));
        self.cells.resize(rows, Vec::new());
        for row in &mut self.cells {
            row.resize(self.cols, ' ');
        }
    }

    /// Row count.
    pub fn rows(&self) -> usize {
        self.cells.len()
    }

    /// Column count.
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// The character at `(row, col)`.
    pub fn get(&self, row: usize, col: usize) -> char {
        self.cells[row][col]
    }

    /// Overwrite the character at `(row, col)`.
    pub fn set(&mut self, row: usize, col: usize, ch: char) {
        self.cells[row][col] = ch;
    }

    /// Back to plan text, trailing spaces trimmed, one `\n` per row.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        for row in &self.cells {
            let line: String = row.iter().collect();
            out.push_str(line.trim_end());
            out.push('\n');
        }
        out
    }

    fn at(&self, r: isize, c: isize) -> Option<char> {
        if r < 0 || c < 0 || r as usize >= self.rows() || c as usize >= self.cols {
            return None;
        }
        Some(self.cells[r as usize][c as usize])
    }

    fn kind(&self, r: isize, c: isize) -> Option<Kind> {
        self.at(r, c).map(kind_of)
    }

    fn solid(&self, r: isize, c: isize) -> bool {
        matches!(self.kind(r, c), Some(Kind::Wall | Kind::Door))
    }

    fn sockets(&self, r: isize, c: isize) -> [u8; 4] {
        self.at(r, c).and_then(wall_sockets).unwrap_or([0; 4])
    }

    fn walls(&self) -> Vec<(isize, isize)> {
        self.positions(|k| k == Kind::Wall)
    }

    fn positions(&self, want: impl Fn(Kind) -> bool) -> Vec<(isize, isize)> {
        let mut out = Vec::new();
        for (r, row) in self.cells.iter().enumerate() {
            for (c, &ch) in row.iter().enumerate() {
                if want(kind_of(ch)) {
                    out.push((r as isize, c as isize));
                }
            }
        }
        out
    }
}

/// One rule a plan breaks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Issue {
    /// A wall's socket points at something that is not a wall or door.
    BrokenWall { row: usize, col: usize, side: char },
    /// A door with no wall on both sides of it.
    StrayDoor { row: usize, col: usize },
    /// A room that reaches the edge of the plan: a wall or door is missing.
    Leak { row: usize, col: usize },
    /// A room with no chain of doors to the outside.
    Sealed {
        row: usize,
        col: usize,
        cells: usize,
    },
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Issue::BrokenWall { row, col, side } => {
                write!(
                    f,
                    "row {row} col {col}: broken wall, its {side} side connects to nothing"
                )
            }
            Issue::StrayDoor { row, col } => {
                write!(f, "row {row} col {col}: door is not set in a wall")
            }
            Issue::Leak { row, col } => {
                write!(
                    f,
                    "row {row} col {col}: room is open to the outside (missing wall or door)"
                )
            }
            Issue::Sealed { row, col, cells } => {
                write!(
                    f,
                    "row {row} col {col}: room of {cells} cells has no door path to the outside"
                )
            }
        }
    }
}

/// What [`check`] found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    /// Every rule broken, in reading order per kind.
    pub issues: Vec<Issue>,
    /// Enclosed rooms found.
    pub rooms: usize,
}

impl Report {
    /// True when no rule is broken.
    pub fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }
}

/// Open regions of a plan, split by walls and doors.
struct Regions {
    id: Vec<Vec<usize>>,
    outside: Vec<bool>,
    interior: Vec<usize>,
    first: Vec<(usize, usize)>,
}

const SOLID: usize = usize::MAX;

impl Regions {
    fn of(g: &Grid) -> Self {
        let mut rg = Self {
            id: vec![vec![SOLID; g.cols]; g.rows()],
            outside: vec![],
            interior: vec![],
            first: vec![],
        };
        for (r0, c0) in g.positions(|k| k == Kind::Open) {
            if rg.id[r0 as usize][c0 as usize] != SOLID {
                continue;
            }
            let n = rg.outside.len();
            rg.outside.push(false);
            rg.interior.push(0);
            rg.first.push((r0 as usize, c0 as usize));
            let mut first_set = false;
            let mut q = VecDeque::from([(r0, c0)]);
            rg.id[r0 as usize][c0 as usize] = n;
            while let Some((r, c)) = q.pop_front() {
                if !g.cells[r as usize][c as usize].is_whitespace() {
                    rg.interior[n] += 1;
                    if !first_set {
                        rg.first[n] = (r as usize, c as usize);
                        first_set = true;
                    }
                }
                for (dr, dc) in STEP {
                    let (nr, nc) = (r + dr, c + dc);
                    match g.kind(nr, nc) {
                        None => rg.outside[n] = true,
                        Some(Kind::Open) if rg.id[nr as usize][nc as usize] == SOLID => {
                            rg.id[nr as usize][nc as usize] = n;
                            q.push_back((nr, nc));
                        }
                        _ => {}
                    }
                }
            }
        }
        rg
    }

    fn len(&self) -> usize {
        self.outside.len()
    }

    /// Graph node for a cell: the outside node is `len()`; walls are `None`.
    fn node(&self, g: &Grid, r: isize, c: isize) -> Option<usize> {
        match g.kind(r, c) {
            None => Some(self.len()),
            Some(Kind::Open) => {
                let id = self.id[r as usize][c as usize];
                Some(if self.outside[id] { self.len() } else { id })
            }
            _ => None,
        }
    }

    fn is_room(&self, id: usize) -> bool {
        !self.outside[id] && self.interior[id] > 0
    }
}

/// The two cells a wall-line cell at `(r, c)` separates, if it sits in a straight wall line.
fn across(g: &Grid, r: isize, c: isize) -> Option<[(isize, isize); 2]> {
    if g.solid(r, c - 1) && g.solid(r, c + 1) {
        Some([(r - 1, c), (r + 1, c)])
    } else if g.solid(r - 1, c) && g.solid(r + 1, c) {
        Some([(r, c - 1), (r, c + 1)])
    } else {
        None
    }
}

/// Which graph nodes are reachable from the outside through doors.
fn reached(g: &Grid, rg: &Regions) -> Vec<bool> {
    let n = rg.len();
    let mut adj = vec![Vec::new(); n + 1];
    for (r, c) in g.positions(|k| k == Kind::Door) {
        if let Some([a, b]) = across(g, r, c) {
            if let (Some(x), Some(y)) = (rg.node(g, a.0, a.1), rg.node(g, b.0, b.1)) {
                adj[x].push(y);
                adj[y].push(x);
            }
        }
    }
    let mut seen = vec![false; n + 1];
    seen[n] = true;
    let mut q = VecDeque::from([n]);
    while let Some(x) = q.pop_front() {
        for &y in &adj[x] {
            if !seen[y] {
                seen[y] = true;
                q.push_back(y);
            }
        }
    }
    seen
}

/// Check every socket, door and room in a plan.
pub fn check(g: &Grid) -> Report {
    let mut issues = Vec::new();
    for (r, c) in g.walls() {
        let s = g.sockets(r, c);
        for d in 0..4 {
            if s[d] == 0 {
                continue;
            }
            let (nr, nc) = (r + STEP[d].0, c + STEP[d].1);
            let ok = match g.kind(nr, nc) {
                Some(Kind::Door) => true,
                Some(Kind::Wall) => g.sockets(nr, nc)[OPP[d]] > 0,
                _ => false,
            };
            if !ok {
                issues.push(Issue::BrokenWall {
                    row: r as usize,
                    col: c as usize,
                    side: SIDE[d],
                });
            }
        }
    }
    for (r, c) in g.positions(|k| k == Kind::Door) {
        if across(g, r, c).is_none() {
            issues.push(Issue::StrayDoor {
                row: r as usize,
                col: c as usize,
            });
        }
    }
    let rg = Regions::of(g);
    let seen = reached(g, &rg);
    let mut rooms = 0;
    for id in 0..rg.len() {
        let (row, col) = rg.first[id];
        if rg.outside[id] && rg.interior[id] > 0 {
            issues.push(Issue::Leak { row, col });
        } else if rg.is_room(id) {
            rooms += 1;
            if !seen[id] {
                issues.push(Issue::Sealed {
                    row,
                    col,
                    cells: rg.interior[id],
                });
            }
        }
    }
    Report { issues, rooms }
}

/// What [`fix`] changed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FixStats {
    /// Wall gaps closed with a door.
    pub gaps_bridged: u32,
    /// Wall glyphs redrawn to match their neighbours.
    pub walls_redrawn: u32,
    /// Doors cut into sealed rooms.
    pub doors_added: u32,
}

/// Repair a plan in place: bridge gaps, redraw walls, cut doors. Run [`check`]
/// afterwards for anything left (a gap wider than [`MAX_GAP`] stays a leak).
pub fn fix(g: &mut Grid) -> FixStats {
    let gaps_bridged = bridge_gaps(g);
    let walls_redrawn = autotile(g);
    let doors_added = add_doors(g);
    FixStats {
        gaps_bridged,
        walls_redrawn,
        doors_added,
    }
}

/// Close each short run of open cells that two wall sockets point into:
/// a door in the middle, wall either side. Returns gaps closed.
pub fn bridge_gaps(g: &mut Grid) -> u32 {
    let mut closed = 0;
    for (r, c) in g.walls() {
        for (d, fill_single, fill_double) in [(E, '─', '═'), (S, '│', '║')] {
            let weight = g.sockets(r, c)[d];
            if weight == 0 {
                continue;
            }
            let (dr, dc) = STEP[d];
            let mut len = 0;
            while len < MAX_GAP
                && g.kind(r + dr * (len as isize + 1), c + dc * (len as isize + 1))
                    == Some(Kind::Open)
            {
                len += 1;
            }
            let (er, ec) = (r + dr * (len as isize + 1), c + dc * (len as isize + 1));
            if len == 0 || g.kind(er, ec) != Some(Kind::Wall) || g.sockets(er, ec)[OPP[d]] == 0 {
                continue;
            }
            let fill = if weight == 2 {
                fill_double
            } else {
                fill_single
            };
            for k in 1..=len {
                let ch = if k == 1 + (len - 1) / 2 { DOOR } else { fill };
                g.set(
                    (r + dr * k as isize) as usize,
                    (c + dc * k as isize) as usize,
                    ch,
                );
            }
            closed += 1;
        }
    }
    closed
}

/// Redraw every wall glyph from which neighbours are walls or doors, keeping
/// its stroke weight. Returns glyphs changed.
pub fn autotile(g: &mut Grid) -> u32 {
    let mut changed = 0;
    for (r, c) in g.walls() {
        let weight = g.sockets(r, c).into_iter().max().unwrap_or(1).max(1);
        let on: [bool; 4] = std::array::from_fn(|d| g.solid(r + STEP[d].0, c + STEP[d].1));
        let glyph = wall_glyph(on.map(|x| if x { weight } else { 0 }))
            .or_else(|| wall_glyph(on.map(u8::from)));
        if let Some(ch) = glyph {
            if ch != g.get(r as usize, c as usize) {
                g.set(r as usize, c as usize, ch);
                changed += 1;
            }
        }
    }
    changed
}

/// Cut one door per sealed room, into the middle of the longest straight wall
/// it shares with a reachable room (the outside only when there is no such
/// room), until every room is reachable. Returns doors cut.
pub fn add_doors(g: &mut Grid) -> u32 {
    let mut added = 0;
    for _ in 0..crate::MAX_COLS * crate::MAX_ROWS {
        let rg = Regions::of(g);
        let seen = reached(g, &rg);
        let Some(room) = (0..rg.len()).find(|&id| rg.is_room(id) && !seen[id]) else {
            break;
        };
        let (mut inner, mut outer) = (Vec::new(), Vec::new());
        for (r, c) in g.walls() {
            let Some([a, b]) = across(g, r, c) else {
                continue;
            };
            let (Some(x), Some(y)) = (rg.node(g, a.0, a.1), rg.node(g, b.0, b.1)) else {
                continue;
            };
            let other = if x == room {
                y
            } else if y == room {
                x
            } else {
                continue;
            };
            if !seen[other] {
                continue;
            }
            let horizontal = a.1 == b.1;
            let key = if horizontal { (0, r, c) } else { (1, c, r) };
            if other == rg.len() {
                outer.push(key)
            } else {
                inner.push(key)
            }
        }
        let pick = if inner.is_empty() { outer } else { inner };
        let Some((axis, line, pos)) = middle_of_longest_run(pick) else {
            break;
        };
        let (r, c) = if axis == 0 { (line, pos) } else { (pos, line) };
        g.set(r as usize, c as usize, DOOR);
        added += 1;
    }
    added
}

fn middle_of_longest_run(mut keys: Vec<(u8, isize, isize)>) -> Option<(u8, isize, isize)> {
    keys.sort_unstable();
    let mut best: &[(u8, isize, isize)] = &[];
    let mut start = 0;
    for i in 1..=keys.len() {
        let breaks = i == keys.len() || {
            let (p, q) = (keys[i - 1], keys[i]);
            p.0 != q.0 || p.1 != q.1 || p.2 + 1 != q.2
        };
        if breaks {
            if i - start > best.len() {
                best = &keys[start..i];
            }
            start = i;
        }
    }
    best.get(best.len() / 2).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_wall_glyph_round_trips_through_its_sockets() {
        for (ch, s) in WALLS {
            assert_eq!(wall_glyph(s), Some(ch));
        }
    }

    #[test]
    fn closed_room_with_door_is_valid() {
        let g = Grid::parse("┌──┐\n│..│\n└+─┘\n");
        let rep = check(&g);
        assert!(rep.is_valid(), "{:?}", rep.issues);
        assert_eq!(rep.rooms, 1);
    }

    #[test]
    fn gap_is_a_leak_and_broken_walls_then_fix_bridges_it() {
        let mut g = Grid::parse("┌──┐\n│..│\n└.─┘\n");
        let rep = check(&g);
        assert!(rep.issues.contains(&Issue::Leak { row: 1, col: 1 }));
        assert!(rep.issues.contains(&Issue::BrokenWall {
            row: 2,
            col: 0,
            side: 'E'
        }));
        let st = fix(&mut g);
        assert_eq!(st.gaps_bridged, 1);
        assert_eq!(g.to_text(), "┌──┐\n│..│\n└+─┘\n");
        assert!(check(&g).is_valid());
    }

    #[test]
    fn sealed_room_gets_a_door_and_wrong_junction_is_redrawn() {
        let mut g = Grid::parse("┌────┼────┐\n│....│....│\n└──+─┴────┘\n");
        let rep = check(&g);
        assert!(rep.issues.contains(&Issue::BrokenWall {
            row: 0,
            col: 5,
            side: 'N'
        }));
        assert!(rep.issues.iter().any(|i| matches!(i, Issue::Sealed { .. })));
        let st = fix(&mut g);
        assert_eq!((st.walls_redrawn, st.doors_added), (1, 1));
        assert_eq!(
            g.to_text(),
            "┌────┬────┐\n│....│....│\n└──+─┴────┘\n".replace("│....│....│", "│....+....│")
        );
        assert!(check(&g).is_valid());
    }
}
