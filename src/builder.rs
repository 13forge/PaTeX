//! The builder: drop a piece where the cursor is.
//!
//! [`drop_at`] takes a piece and a cursor cell (a mouse click in an editor, a
//! `--at row,col` on the command line). The piece snaps up to [`SNAP`] cells
//! so its walls share existing walls instead of running alongside them. Then
//! the socket rules ([`crate::sockets::fix`]) redraw every wall and cut the
//! doors, so the plan stays valid after every drop.
//!
//! [`generate`] is the same call with seeded random cursors.

use crate::sockets::{self, FixStats, Grid};

/// How far a drop may slide from the cursor to share a wall.
pub const SNAP: i64 = 2;

/// Something to drop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Piece {
    /// A room `w x h` cells, walls included (at least 3 x 3).
    Room { w: usize, h: usize },
    /// A block of fill (`▓` counter, `█` pillar, `@` anchor...) placed on floor.
    Fill { w: usize, h: usize, ch: char },
}

impl Piece {
    fn size(self) -> (i64, i64) {
        match self {
            Piece::Room { w, h } | Piece::Fill { w, h, .. } => (w as i64, h as i64),
        }
    }
}

/// Why a drop was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    /// The piece is too small to be a room (needs 3 x 3) or empty.
    TooSmall,
    /// No position within [`SNAP`] of the cursor fits on the sheet and the plan.
    NoRoom,
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refused::TooSmall => write!(f, "piece too small (a room needs at least 3x3)"),
            Refused::NoRoom => write!(
                f,
                "no space for the piece within {SNAP} cells of the cursor"
            ),
        }
    }
}

/// Where a drop landed and what the sockets fixed after it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dropped {
    /// Top-left row of the piece as placed.
    pub row: usize,
    /// Top-left column of the piece as placed.
    pub col: usize,
    /// Wall cells shared with what was already there.
    pub shared: u32,
    /// What the socket pass changed.
    pub fixed: FixStats,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tile {
    Empty,
    Floor,
    Wall,
}

fn tile(g: &Grid, r: i64, c: i64) -> Option<Tile> {
    if r < 0 || c < 0 || r as usize >= g.rows() || c as usize >= g.cols() {
        return None;
    }
    let ch = g.get(r as usize, c as usize);
    Some(if ch == ' ' {
        Tile::Empty
    } else if ch == sockets::DOOR || sockets::wall_sockets(ch).is_some() {
        Tile::Wall
    } else {
        Tile::Floor
    })
}

/// Score a room at top-left `(r0, c0)`: `Some(shared wall cells)` if it fits.
/// Floor must land on empty ground; walls on empty ground or existing walls;
/// and no new wall may run alongside an old one (a double-thick wall).
fn room_fit(g: &Grid, r0: i64, c0: i64, w: i64, h: i64) -> Option<u32> {
    let inside = |r: i64, c: i64| (r0..r0 + h).contains(&r) && (c0..c0 + w).contains(&c);
    let mut shared = 0;
    for r in r0..r0 + h {
        for c in c0..c0 + w {
            let t = tile(g, r, c)?;
            let border = r == r0 || r == r0 + h - 1 || c == c0 || c == c0 + w - 1;
            match (border, t) {
                (false, Tile::Empty) => {}
                (false, _) | (true, Tile::Floor) => return None,
                (true, Tile::Wall) => shared += 1,
                (true, Tile::Empty) => {
                    for (dr, dc) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                        if !inside(r + dr, c + dc) && tile(g, r + dr, c + dc) == Some(Tile::Wall) {
                            return None;
                        }
                    }
                }
            }
        }
    }
    Some(shared)
}

fn fill_fit(g: &Grid, r0: i64, c0: i64, w: i64, h: i64) -> Option<u32> {
    for r in r0..r0 + h {
        for c in c0..c0 + w {
            if g.get_checked(r, c)? != '.' {
                return None;
            }
        }
    }
    Some(0)
}

impl Grid {
    fn get_checked(&self, r: i64, c: i64) -> Option<char> {
        tile(self, r, c).map(|_| self.get(r as usize, c as usize))
    }
}

/// Drop `piece` centred on the cursor cell `(row, col)`, snapped to share walls.
///
/// Every position within [`SNAP`] cells is scored; the one sharing the most
/// wall wins, ties going to the one nearest the cursor, then reading order.
/// On success the plan has been re-tiled and every room has a door path.
pub fn drop_at(g: &mut Grid, piece: Piece, row: usize, col: usize) -> Result<Dropped, Refused> {
    let (w, h) = piece.size();
    let room = matches!(piece, Piece::Room { .. });
    if w < 1 || h < 1 || (room && (w < 3 || h < 3)) {
        return Err(Refused::TooSmall);
    }
    let (r_aim, c_aim) = (row as i64 - (h - 1) / 2, col as i64 - (w - 1) / 2);
    let mut best: Option<(u32, i64, i64, i64)> = None;
    for dr in -SNAP..=SNAP {
        for dc in -SNAP..=SNAP {
            let (r0, c0) = (r_aim + dr, c_aim + dc);
            let fit = if room {
                room_fit(g, r0, c0, w, h)
            } else {
                fill_fit(g, r0, c0, w, h)
            };
            let Some(shared) = fit else { continue };
            let dist = dr.abs() + dc.abs();
            let better = match best {
                None => true,
                Some((s, d, br, bc)) => (shared, -dist, -r0, -c0) > (s, -d, -br, -bc),
            };
            if better {
                best = Some((shared, dist, r0, c0));
            }
        }
    }
    let (shared, _, r0, c0) = best.ok_or(Refused::NoRoom)?;
    for r in r0..r0 + h {
        for c in c0..c0 + w {
            let (ru, cu) = (r as usize, c as usize);
            match piece {
                Piece::Fill { ch, .. } => g.set(ru, cu, ch),
                Piece::Room { .. } => {
                    let border = r == r0 || r == r0 + h - 1 || c == c0 || c == c0 + w - 1;
                    if !border {
                        g.set(ru, cu, '.');
                    } else if tile(g, r, c) == Some(Tile::Empty) {
                        g.set(ru, cu, '─');
                    }
                }
            }
        }
    }
    // Draw the new walls first: placeholder `─` sockets would read as a gap to bridge.
    let drawn = sockets::autotile(g);
    let mut fixed = sockets::fix(g);
    fixed.walls_redrawn += drawn;
    Ok(Dropped {
        row: r0 as usize,
        col: c0 as usize,
        shared,
        fixed,
    })
}

/// Random-plan settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GenOptions {
    /// Plan width, clamped to `8..=71`.
    pub cols: usize,
    /// Plan height, clamped to `6..=48`.
    pub rows: usize,
    /// Rooms to drop (fewer if the sheet runs out of space).
    pub rooms: usize,
    /// Seed: same seed, same plan.
    pub seed: u64,
}

impl Default for GenOptions {
    fn default() -> Self {
        Self {
            cols: 31,
            rows: 15,
            rooms: 4,
            seed: 1,
        }
    }
}

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        let s = seed ^ 0x9E37_79B9_7F4A_7C15;
        Self(if s == 0 { 1 } else { s })
    }

    /// Uniform in `lo..=hi` (xorshift64*).
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        if hi <= lo {
            return lo;
        }
        lo + (x.wrapping_mul(0x2545_F491_4F6C_DD1D) % (hi - lo + 1) as u64) as i64
    }
}

/// A random plan: rooms dropped at seeded cursor positions. The first lands in
/// the middle; each later cursor lands on the edge of an earlier room, so the
/// snap joins them into one building.
pub fn generate(o: &GenOptions) -> Grid {
    let cols = o.cols.clamp(8, crate::MAX_COLS);
    let rows = o.rows.clamp(6, crate::MAX_ROWS);
    let mut g = Grid::blank(cols, rows);
    let mut rng = Rng::new(o.seed);
    let mut rooms: Vec<Dropped> = Vec::new();
    let mut sizes: Vec<(i64, i64)> = Vec::new();
    let mut budget = o.rooms.max(1) * 48;
    while rooms.len() < o.rooms.max(1) && budget > 0 {
        budget -= 1;
        let (w, h) = (
            rng.range(6, 12.min(cols as i64)),
            rng.range(5, 8.min(rows as i64)),
        );
        let (r, c) = match rooms.len() {
            0 => (rows as i64 / 2, cols as i64 / 2),
            n => {
                let k = rng.range(0, n as i64 - 1) as usize;
                let (at, (pw, ph)) = (rooms[k], sizes[k]);
                let (top, left) = (at.row as i64, at.col as i64);
                // Aim one half-piece beyond a random side of room k.
                match rng.range(0, 3) {
                    0 => (top + rng.range(0, ph - 1), left + pw - 1 + (w - 1) / 2),
                    1 => (top + rng.range(0, ph - 1), left - w / 2),
                    2 => (top + ph - 1 + (h - 1) / 2, left + rng.range(0, pw - 1)),
                    _ => (top - h / 2, left + rng.range(0, pw - 1)),
                }
            }
        };
        if r < 0 || c < 0 {
            continue;
        }
        if let Ok(d) = drop_at(
            &mut g,
            Piece::Room {
                w: w as usize,
                h: h as usize,
            },
            r as usize,
            c as usize,
        ) {
            rooms.push(d);
            sizes.push((w, h));
        }
    }
    g
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drop_on_empty_sheet_makes_one_room_with_a_door() {
        let mut g = Grid::blank(12, 8);
        let d = drop_at(&mut g, Piece::Room { w: 6, h: 4 }, 4, 6).unwrap();
        assert_eq!((d.row, d.col, d.shared, d.fixed.doors_added), (3, 4, 0, 1));
        let rep = sockets::check(&g);
        assert!(
            rep.is_valid() && rep.rooms == 1,
            "{:?}\n{}",
            rep.issues,
            g.to_text()
        );
    }

    #[test]
    fn second_drop_snaps_onto_the_shared_wall() {
        let mut g = Grid::blank(20, 8);
        drop_at(&mut g, Piece::Room { w: 6, h: 5 }, 3, 4).unwrap();
        // Cursor one cell off: the snap slides it to share the east wall.
        let d = drop_at(&mut g, Piece::Room { w: 6, h: 5 }, 3, 10).unwrap();
        assert_eq!(d.shared, 5, "\n{}", g.to_text());
        let rep = sockets::check(&g);
        assert!(
            rep.is_valid() && rep.rooms == 2,
            "{:?}\n{}",
            rep.issues,
            g.to_text()
        );
    }

    #[test]
    fn fill_lands_only_on_floor() {
        let mut g = Grid::blank(12, 8);
        drop_at(&mut g, Piece::Room { w: 8, h: 6 }, 4, 6).unwrap();
        assert!(drop_at(
            &mut g,
            Piece::Fill {
                w: 2,
                h: 1,
                ch: '▓'
            },
            3,
            5
        )
        .is_ok());
        assert_eq!(
            drop_at(
                &mut g,
                Piece::Fill {
                    w: 30,
                    h: 1,
                    ch: '▓'
                },
                3,
                5
            ),
            Err(Refused::NoRoom)
        );
        assert!(sockets::check(&g).is_valid());
    }

    #[test]
    fn every_seed_builds_a_valid_plan() {
        for seed in 0..300 {
            for (cols, rows, rooms) in [(31, 15, 4), (71, 48, 12), (20, 10, 3)] {
                let g = generate(&GenOptions {
                    cols,
                    rows,
                    rooms,
                    seed,
                });
                let rep = sockets::check(&g);
                assert!(
                    rep.is_valid(),
                    "seed {seed} {cols}x{rows}: {:?}\n{}",
                    rep.issues,
                    g.to_text()
                );
                assert!(rep.rooms >= 1);
            }
        }
    }

    #[test]
    fn same_seed_same_plan() {
        let o = GenOptions {
            seed: 42,
            ..GenOptions::default()
        };
        assert_eq!(generate(&o), generate(&o));
        assert_ne!(generate(&o), generate(&GenOptions { seed: 43, ..o }));
    }
}
