//! PaTeX isometric renderer.
//!
//! Reads a box-drawing ASCII plan (`┌─┐│└┘`, `.` floor, `▓` fills) and renders
//! it as a true 2:1 dimetric axonometric: every cell is a block with three
//! shaded faces, drawn back to front. Zero dependencies, integer arithmetic
//! only, so the same plan always produces the same bytes.
//!
//! Spec: `06_PATEX_5D_GEOMETRIC_TYPESETTING.md` §5 (axonometry) in this repo.

mod png;

pub use png::encode_png;

/// Pane width ceiling (spec: 71-column viewport).
pub const MAX_COLS: usize = 71;
/// Pane height ceiling.
pub const MAX_ROWS: usize = 48;

/// Projection slope run (2:1 dimetric).
pub const RUN: i64 = 2;
/// Projection slope rise.
pub const RISE: i64 = 1;

/// Sub-samples per pixel side; coverage is scored in ninths.
const SUB: i64 = 3;
const DEN: u32 = (SUB * SUB) as u32;

/// The 44 single / double / mixed box-drawing glyphs (spec §3).
pub const BOX_GLYPHS: &str = "─│┌┐└┘├┤┬┴┼═║╔╗╚╝╠╣╦╩╬╒╓╕╖╘╙╛╜╞╟╡╢╤╥╧╨╪╫╵╶╷╴";

/// Which lane a glyph lives on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lane {
    /// Box-drawing walls.
    Topology,
    /// Occlusion fills: `· ░ ▒ ▓ █`.
    Material,
    /// Flat marks: `. ◆ @ ~ #`.
    Mark,
}

/// One bound cell of a plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    /// The cell's lane.
    pub lane: Lane,
    /// Occlusion in permyriad (`0..=10_000`); only meaningful on the material lane.
    pub density_pmy: u16,
}

/// The cell a glyph binds to, or `None` (space, letters, brackets: not drawn).
pub fn classify(ch: char) -> Option<Cell> {
    if BOX_GLYPHS.contains(ch) {
        return Some(Cell { lane: Lane::Topology, density_pmy: 10_000 });
    }
    let material = match ch {
        '·' => Some(0),
        '░' => Some(2_500),
        '▒' => Some(5_000),
        '▓' => Some(7_500),
        '█' => Some(10_000),
        _ => None,
    };
    if let Some(d) = material {
        return Some(Cell { lane: Lane::Material, density_pmy: d });
    }
    match ch {
        '.' | '◆' | '@' | '~' | '#' => Some(Cell { lane: Lane::Mark, density_pmy: 0 }),
        _ => None,
    }
}

/// What parsing a plan consumed and dropped.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ParseStats {
    /// Cells bound to a lane.
    pub bound: u32,
    /// Non-space glyphs with no binding (labels etc.), left undrawn.
    pub unbound: u32,
    /// Characters past column 71.
    pub cols_overflowed: u32,
    /// Lines past row 48.
    pub rows_overflowed: u32,
}

/// A parsed plan: a `rows x cols` grid of optional cells.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    cells: Vec<Vec<Option<Cell>>>,
    cols: usize,
}

impl Plan {
    /// Parse plan text. If it holds a `\topology{ ... }` block (the `.ptex`
    /// form), only the block body is read; otherwise every line is the plan.
    pub fn parse(src: &str) -> (Self, ParseStats) {
        let mut st = ParseStats::default();
        let mut cells = Vec::new();
        let mut cols = 0;
        for line in body_lines(src) {
            if cells.len() >= MAX_ROWS {
                st.rows_overflowed += 1;
                continue;
            }
            let mut row = Vec::new();
            for ch in line.chars() {
                if row.len() >= MAX_COLS {
                    st.cols_overflowed += 1;
                    continue;
                }
                let c = classify(ch);
                match c {
                    Some(_) => st.bound += 1,
                    None if !ch.is_whitespace() => st.unbound += 1,
                    None => {}
                }
                row.push(c);
            }
            cols = cols.max(row.len());
            cells.push(row);
        }
        (Self { cells, cols }, st)
    }

    /// Column count (widest row).
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Row count.
    pub fn rows(&self) -> usize {
        self.cells.len()
    }

    /// The cell at `(col, row)`, if bound.
    pub fn cell(&self, col: usize, row: usize) -> Option<Cell> {
        self.cells.get(row).and_then(|r| r.get(col)).copied().flatten()
    }
}

fn body_lines(src: &str) -> Vec<&str> {
    let mut lines = src.lines();
    if src.contains("\\topology{") {
        let mut out = Vec::new();
        for l in lines.by_ref() {
            if l.contains("\\topology{") {
                break;
            }
        }
        for l in lines {
            if l.trim() == "}" {
                break;
            }
            out.push(l);
        }
        return out;
    }
    lines.collect()
}

/// Block height per lane, in elevation units.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Extrude {
    /// Wall height.
    pub topology: u8,
    /// Mark height (0 = flat on the floor).
    pub mark: u8,
    /// Height of a fully occluding fill; lighter fills scale by density.
    pub material: u8,
}

impl Extrude {
    /// Walls stand 3, floor lies flat, fills rise with density.
    pub const CANONICAL: Self = Self { topology: 3, mark: 0, material: 2 };

    /// Height of one cell.
    pub fn height_of(&self, c: Cell) -> u8 {
        match c.lane {
            Lane::Topology => self.topology,
            Lane::Mark => self.mark,
            Lane::Material => ((self.material as u32 * c.density_pmy as u32) / 10_000) as u8,
        }
    }

    /// Tallest height under this profile.
    pub fn max_height(&self) -> u8 {
        self.topology.max(self.mark).max(self.material)
    }
}

/// Per-lane ink, plus the sheet colour. `[r, g, b]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    /// Wall ink.
    pub topology: [u8; 3],
    /// Fill ink.
    pub material: [u8; 3],
    /// Floor / mark ink.
    pub mark: [u8; 3],
    /// Sheet background.
    pub background: [u8; 3],
}

impl Palette {
    /// Cool walls, warm floor, dim fills on a dark sheet.
    pub const CANONICAL: Self = Self {
        topology: [92, 132, 196],
        material: [120, 104, 92],
        mark: [214, 160, 104],
        background: [18, 20, 26],
    };

    /// Architect's blueprint: pale cyan walls and fills on blueprint blue.
    pub const BLUEPRINT: Self = Self {
        topology: [226, 240, 255],
        material: [150, 200, 240],
        mark: [42, 92, 168],
        background: [16, 58, 120],
    };

    /// A named palette: `canonical` or `blueprint`.
    pub fn named(name: &str) -> Option<Self> {
        match name {
            "canonical" | "default" => Some(Self::CANONICAL),
            "blueprint" => Some(Self::BLUEPRINT),
            _ => None,
        }
    }

    fn ink(&self, lane: Lane) -> [u8; 3] {
        match lane {
            Lane::Topology => self.topology,
            Lane::Material => self.material,
            Lane::Mark => self.mark,
        }
    }
}

/// One of a block's three visible faces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Face {
    /// South face, falling away to the left.
    Left,
    /// Top face.
    Top,
    /// East face, falling away to the right.
    Right,
}

impl Face {
    /// Every face, in draw order.
    pub const ALL: [Self; 3] = [Self::Left, Self::Top, Self::Right];

    /// Shade applied to the ink, in permyriad. Top catches the light.
    pub const fn shade_pmy(self) -> u32 {
        match self {
            Self::Top => 10_000,
            Self::Right => 6_600,
            Self::Left => 4_200,
        }
    }
}

/// Render settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    /// Tile width in pixels (a flat tile is `tile_w x tile_w / 2`).
    pub tile_w: i64,
    /// Pixels per elevation unit.
    pub elev: i64,
    /// Margin around the drawing, pixels.
    pub pad: i64,
    /// Block heights.
    pub extrude: Extrude,
    /// Colours.
    pub palette: Palette,
}

impl Default for Options {
    fn default() -> Self {
        Self { tile_w: 16, elev: 8, pad: 8, extrude: Extrude::CANONICAL, palette: Palette::CANONICAL }
    }
}

/// An RGBA8 image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    /// Width, pixels.
    pub width: u32,
    /// Height, pixels.
    pub height: u32,
    /// Row-major RGBA bytes, `width * height * 4`.
    pub data: Vec<u8>,
}

/// What one render drew.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenderStats {
    /// Cells drawn.
    pub cells: u32,
    /// Faces that put ink on the sheet.
    pub faces: u32,
}

/// Project a cell-space corner `(x, z, height)` to sub-pixel units.
fn corner(cx: i64, cz: i64, cy: i64, origin: (i64, i64), tile_w: i64, elev: i64) -> (i64, i64) {
    let half_w = tile_w / 2;
    let half_h = half_w * RISE / RUN;
    (
        (origin.0 + (cx - cz) * half_w) * SUB,
        (origin.1 + (cx + cz) * half_h - cy * elev) * SUB,
    )
}

fn face_corners(face: Face, col: i64, row: i64, h: i64) -> [(i64, i64, i64); 4] {
    match face {
        Face::Top => [(col, row, h), (col + 1, row, h), (col + 1, row + 1, h), (col, row + 1, h)],
        Face::Right => [(col + 1, row, h), (col + 1, row + 1, h), (col + 1, row + 1, 0), (col + 1, row, 0)],
        Face::Left => [(col, row + 1, h), (col + 1, row + 1, h), (col + 1, row + 1, 0), (col, row + 1, 0)],
    }
}

/// Point in convex quad by consistent cross-product sign; edges count as inside.
fn inside_quad(q: &[(i64, i64); 4], px: i64, py: i64) -> bool {
    let (mut neg, mut pos) = (false, false);
    for i in 0..4 {
        let (ax, ay) = q[i];
        let (bx, by) = q[(i + 1) % 4];
        let cross = (bx - ax) * (py - ay) - (by - ay) * (px - ax);
        neg |= cross < 0;
        pos |= cross > 0;
    }
    !(neg && pos)
}

/// Render a plan to an image sized to fit it.
pub fn render(plan: &Plan, opt: &Options) -> (Image, RenderStats) {
    let (cols, rows) = (plan.cols() as i64, plan.rows() as i64);
    let half_w = opt.tile_w / 2;
    let half_h = half_w * RISE / RUN;
    let lift = opt.extrude.max_height() as i64 * opt.elev;
    let width = ((cols + rows) * half_w + 2 * opt.pad).max(1) as u32;
    let height = ((cols + rows) * half_h + lift + 2 * opt.pad).max(1) as u32;
    let origin = (rows * half_w + opt.pad, lift + opt.pad);

    let bg = opt.palette.background;
    let mut img = Image { width, height, data: Vec::with_capacity((width * height * 4) as usize) };
    for _ in 0..width * height {
        img.data.extend_from_slice(&[bg[0], bg[1], bg[2], 255]);
    }

    // Painter's order: col + row ascending, farthest first.
    let mut order = Vec::new();
    for row in 0..plan.rows() {
        for col in 0..plan.cols() {
            if plan.cell(col, row).is_some() {
                order.push((col + row, col, row));
            }
        }
    }
    order.sort_unstable();

    let mut st = RenderStats::default();
    for (_, col, row) in order {
        let cell = plan.cell(col, row).expect("ordered cells are bound");
        let h = opt.extrude.height_of(cell) as i64;
        let ink = opt.palette.ink(cell.lane);
        let mut drew = false;
        for face in Face::ALL {
            if h == 0 && face != Face::Top {
                continue;
            }
            let cs = face_corners(face, col as i64, row as i64, h);
            let q = cs.map(|(x, z, y)| corner(x, z, y, origin, opt.tile_w, opt.elev));
            if paint_quad(&mut img, &q, ink, face.shade_pmy()) {
                st.faces += 1;
                drew = true;
            }
        }
        if drew {
            st.cells += 1;
        }
    }
    (img, st)
}

/// Fill one face with 3x3 sub-sample coverage. Returns whether it inked anything.
fn paint_quad(img: &mut Image, q: &[(i64, i64); 4], ink: [u8; 3], shade: u32) -> bool {
    let (mut x0, mut y0, mut x1, mut y1) = (i64::MAX, i64::MAX, i64::MIN, i64::MIN);
    for &(x, y) in q {
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    }
    let fill = ink.map(|c| (c as u32 * shade / 10_000) as u8);
    let half = (SUB - 1) / 2;
    let mut painted = false;
    for py in y0.div_euclid(SUB).max(0)..(y1.div_euclid(SUB) + 1).min(img.height as i64) {
        for px in x0.div_euclid(SUB).max(0)..(x1.div_euclid(SUB) + 1).min(img.width as i64) {
            let (cx, cy) = (px * SUB + half, py * SUB + half);
            let mut cov = 0u32;
            for sy in -half..=half {
                for sx in -half..=half {
                    cov += inside_quad(q, cx + sx, cy + sy) as u32;
                }
            }
            if cov == 0 {
                continue;
            }
            let at = ((py as u32 * img.width + px as u32) * 4) as usize;
            for k in 0..3 {
                let a = img.data[at + k] as u32;
                img.data[at + k] = ((a * (DEN - cov) + fill[k] as u32 * cov) / DEN) as u8;
            }
            painted = true;
        }
    }
    painted
}

/// FNV-1a 64 over the image bytes: a cheap fingerprint for golden checks.
pub fn fingerprint(img: &Image) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in &img.data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}
