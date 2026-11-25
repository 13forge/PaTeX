//! `patex` CLI: check, fix, build and render ASCII plans.

use std::collections::HashMap;
use std::io::Read;
use std::process::ExitCode;

const USAGE: &str = "\
patex - check, fix, build and render box-drawing ASCII plans

USAGE:
    patex render <plan> [-o out.png] [--fix] [--tile N] [--elev N] [--pad N] [--style S]
    patex check  <plan>
    patex fix    <plan> [-o fixed.txt]
    patex drop   <plan> --at ROW,COL (--room WxH | --fill WxH [--glyph C]) [-o out.txt]
    patex gen    [--seed N] [--cols N] [--rows N] [--rooms N] [-o plan.txt]
    patex <plan> [render options]          (same as render)

    <plan>       a plan file, or '-' for stdin. A .ptex file's \\topology{...} body is used.

COMMANDS:
    render   draw the plan as a 2:1 isometric PNG (default out: plan name .png).
             --fix repairs the plan in memory first. --style canonical|blueprint
    check    list broken walls, stray doors, leaks and sealed rooms. Exit 0 valid, 1 not.
    fix      bridge wall gaps with doors, redraw walls, cut doors into sealed rooms.
             Prints the plan (or writes -o). Exit 0 if valid afterwards.
    drop     drop a piece centred on cell ROW,COL (0-based), snapped to share walls,
             then fix. --room makes a room; --fill puts a block (default '▓') on floor.
             The sheet grows to at least --cols x --rows (default 31x15); start
             from nothing with '-' and empty stdin.
    gen      a random valid plan: rooms dropped at seeded cursor positions.

GLYPHS:
    walls  ┌ ─ ┐ │ └ ┘ ├ ┤ ┬ ┴ ┼ and double ╔ ═ ╗ ║ ...   (height 3)
    door   +                                            (flat, sits in a wall line)
    floor  .                                            (flat)
    fills  ░ ▒ ▓ █                                      (height by density)
    marks  ◆ @ ~ #                                      (flat)
    space and letters are left empty";

const VALUE_FLAGS: &[&str] = &[
    "-o", "--tile", "--elev", "--pad", "--style", "--seed", "--cols", "--rows", "--rooms", "--at",
    "--room", "--fill", "--glyph",
];

struct Args {
    pos: Vec<String>,
    val: HashMap<String, String>,
    fix: bool,
}

impl Args {
    fn parse(raw: &[String]) -> Result<Self, String> {
        let mut a = Self {
            pos: vec![],
            val: HashMap::new(),
            fix: false,
        };
        let mut it = raw.iter();
        while let Some(s) = it.next() {
            if s == "--fix" {
                a.fix = true;
            } else if VALUE_FLAGS.contains(&s.as_str()) {
                let v = it.next().ok_or(format!("{s} needs a value"))?;
                a.val.insert(s.clone(), v.clone());
            } else if s.starts_with('-') && s != "-" {
                return Err(format!("unknown option {s:?}"));
            } else {
                a.pos.push(s.clone());
            }
        }
        Ok(a)
    }

    fn num(&self, flag: &str, default: i64) -> Result<i64, String> {
        match self.val.get(flag) {
            None => Ok(default),
            Some(v) => v
                .parse::<i64>()
                .ok()
                .filter(|n| *n >= 0)
                .ok_or(format!("{flag}: bad number {v:?}")),
        }
    }

    fn pair(&self, flag: &str, sep: char) -> Result<Option<(usize, usize)>, String> {
        let Some(v) = self.val.get(flag) else {
            return Ok(None);
        };
        let bad = || format!("{flag}: expected A{sep}B, got {v:?}");
        let (a, b) = v.split_once(sep).ok_or_else(bad)?;
        Ok(Some((
            a.trim().parse().map_err(|_| bad())?,
            b.trim().parse().map_err(|_| bad())?,
        )))
    }

    fn input(&self) -> Result<(String, String), String> {
        let path = self.pos.first().ok_or("no plan given")?.clone();
        if self.pos.len() > 1 {
            return Err(format!("unexpected argument {:?}", self.pos[1]));
        }
        let text = if path == "-" {
            let mut s = String::new();
            std::io::stdin()
                .read_to_string(&mut s)
                .map_err(|e| e.to_string())?;
            s
        } else {
            std::fs::read_to_string(&path).map_err(|e| format!("{path}: {e}"))?
        };
        Ok((path, text))
    }

    /// Write text to `-o`, or stdout.
    fn emit(&self, text: &str) -> Result<(), String> {
        match self.val.get("-o") {
            Some(p) => std::fs::write(p, text).map_err(|e| format!("{p}: {e}")),
            None => {
                print!("{text}");
                Ok(())
            }
        }
    }
}

fn main() -> ExitCode {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let result = match raw.first().map(String::as_str) {
        None | Some("-h" | "--help" | "help") => {
            println!("{USAGE}");
            Ok(true)
        }
        Some("render") => Args::parse(&raw[1..]).and_then(|a| render(&a)),
        Some("check") => Args::parse(&raw[1..]).and_then(|a| check(&a)),
        Some("fix") => Args::parse(&raw[1..]).and_then(|a| fix(&a)),
        Some("drop") => Args::parse(&raw[1..]).and_then(|a| drop_piece(&a)),
        Some("gen") => Args::parse(&raw[1..]).and_then(|a| generate(&a)),
        Some(_) => Args::parse(&raw).and_then(|a| render(&a)),
    };
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(e) => {
            eprintln!("patex: {e}\n\n{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn print_report(rep: &patex::Report) -> bool {
    for issue in &rep.issues {
        eprintln!("  {issue}");
    }
    if rep.is_valid() {
        eprintln!("valid: {} room(s)", rep.rooms);
    } else {
        eprintln!(
            "invalid: {} issue(s), {} room(s)",
            rep.issues.len(),
            rep.rooms
        );
    }
    rep.is_valid()
}

fn fix_line(st: patex::sockets::FixStats) -> String {
    format!(
        "fixed: {} gap(s) bridged, {} wall glyph(s) redrawn, {} door(s) added",
        st.gaps_bridged, st.walls_redrawn, st.doors_added
    )
}

fn check(a: &Args) -> Result<bool, String> {
    let (_, text) = a.input()?;
    Ok(print_report(&patex::check(&patex::Grid::parse(&text))))
}

fn fix(a: &Args) -> Result<bool, String> {
    let (_, text) = a.input()?;
    let mut g = patex::Grid::parse(&text);
    let st = patex::fix(&mut g);
    a.emit(&g.to_text())?;
    eprintln!("{}", fix_line(st));
    Ok(print_report(&patex::check(&g)))
}

fn drop_piece(a: &Args) -> Result<bool, String> {
    let (_, text) = a.input()?;
    let mut g = patex::Grid::parse(&text);
    // Saved plans trim trailing space, so the sheet is re-grown to its working size.
    g.grow(a.num("--cols", 31)? as usize, a.num("--rows", 15)? as usize);
    let (row, col) = a.pair("--at", ',')?.ok_or("drop needs --at ROW,COL")?;
    let piece = match (a.pair("--room", 'x')?, a.pair("--fill", 'x')?) {
        (Some((w, h)), None) => patex::Piece::Room { w, h },
        (None, Some((w, h))) => {
            let ch = a
                .val
                .get("--glyph")
                .and_then(|s| s.chars().next())
                .unwrap_or('▓');
            patex::Piece::Fill { w, h, ch }
        }
        _ => return Err("drop needs exactly one of --room WxH or --fill WxH".into()),
    };
    let d = match patex::drop_at(&mut g, piece, row, col) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("patex: refused: {e}");
            return Ok(false);
        }
    };
    a.emit(&g.to_text())?;
    eprintln!(
        "dropped at row {} col {} (shared {} wall cells); {}",
        d.row,
        d.col,
        d.shared,
        fix_line(d.fixed)
    );
    Ok(print_report(&patex::check(&g)))
}

fn generate(a: &Args) -> Result<bool, String> {
    if !a.pos.is_empty() {
        return Err(format!("unexpected argument {:?}", a.pos[0]));
    }
    let o = patex::GenOptions {
        cols: a.num("--cols", 31)? as usize,
        rows: a.num("--rows", 15)? as usize,
        rooms: a.num("--rooms", 4)? as usize,
        seed: a.num("--seed", 1)? as u64,
    };
    let g = patex::generate(&o);
    a.emit(&g.to_text())?;
    Ok(print_report(&patex::check(&g)))
}

fn render(a: &Args) -> Result<bool, String> {
    let (input, mut text) = a.input()?;
    let mut opt = patex::Options::default();
    opt.tile_w = a.num("--tile", opt.tile_w)?.max(4);
    opt.elev = a.num("--elev", opt.elev)?;
    opt.pad = a.num("--pad", opt.pad)?;
    if let Some(s) = a.val.get("--style") {
        opt.palette = patex::Palette::named(s).ok_or(format!("unknown style {s:?}"))?;
    }
    if a.fix {
        let mut g = patex::Grid::parse(&text);
        eprintln!("{}", fix_line(patex::fix(&mut g)));
        text = g.to_text();
    }
    let output = a.val.get("-o").cloned().unwrap_or_else(|| {
        if input == "-" {
            "plan.png".into()
        } else {
            std::path::Path::new(&input)
                .with_extension("png")
                .to_string_lossy()
                .into_owned()
        }
    });
    let (plan, ps) = patex::Plan::parse(&text);
    if plan.rows() == 0 || ps.bound == 0 {
        return Err("plan has no drawable cells".into());
    }
    let (img, rs) = patex::render(&plan, &opt);
    std::fs::write(&output, patex::encode_png(&img)).map_err(|e| format!("{output}: {e}"))?;
    eprintln!(
        "{output}: {}x{} px | plan {}x{} | cells {} faces {} | unbound {} overflow {}c/{}r | fnv {:016x}",
        img.width,
        img.height,
        plan.cols(),
        plan.rows(),
        rs.cells,
        rs.faces,
        ps.unbound,
        ps.cols_overflowed,
        ps.rows_overflowed,
        patex::fingerprint(&img)
    );
    Ok(true)
}
