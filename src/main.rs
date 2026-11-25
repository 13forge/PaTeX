//! `patex` CLI: render an ASCII plan to an isometric PNG.

use std::io::Read;
use std::process::ExitCode;

const USAGE: &str = "\
patex - render a box-drawing ASCII plan as a 2:1 isometric PNG

USAGE:
    patex <plan.txt | -> [-o out.png] [--tile N] [--elev N] [--pad N] [--style S]

    <plan.txt>   plan file ('-' reads stdin). A .ptex file's \\topology{...} body is used.
    -o           output PNG (default: plan name with .png)
    --tile N     tile width in px (default 16)
    --elev N     px per height unit (default 8)
    --pad N      margin in px (default 8)
    --style S    canonical (default) or blueprint

GLYPHS:
    walls  ┌ ─ ┐ │ └ ┘ ├ ┤ ┬ ┴ ┼ and double ╔ ═ ╗ ║ ...   (height 3)
    floor  .                                            (flat)
    fills  ░ ▒ ▓ █                                      (height by density)
    marks  ◆ @ ~ #                                      (flat)
    space and letters are left empty";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("patex: {e}\n\n{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let mut input: Option<String> = None;
    let mut output: Option<String> = None;
    let mut opt = patex::Options::default();
    while let Some(a) = args.next() {
        let mut num = |name: &str| -> Result<i64, String> {
            let v = args.next().ok_or(format!("{name} needs a value"))?;
            v.parse::<i64>().ok().filter(|n| *n >= 0).ok_or(format!("{name}: bad number {v:?}"))
        };
        match a.as_str() {
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(());
            }
            "-o" => output = Some(args.next().ok_or("-o needs a path")?),
            "--tile" => opt.tile_w = num("--tile")?.max(4),
            "--elev" => opt.elev = num("--elev")?,
            "--pad" => opt.pad = num("--pad")?,
            "--style" => {
                let s = args.next().ok_or("--style needs a name")?;
                opt.palette = patex::Palette::named(&s).ok_or(format!("unknown style {s:?}"))?;
            }
            _ if input.is_none() => input = Some(a),
            _ => return Err(format!("unexpected argument {a:?}")),
        }
    }
    let input = input.ok_or("no plan given")?;
    let src = if input == "-" {
        let mut s = String::new();
        std::io::stdin().read_to_string(&mut s).map_err(|e| e.to_string())?;
        s
    } else {
        std::fs::read_to_string(&input).map_err(|e| format!("{input}: {e}"))?
    };
    let output = output.unwrap_or_else(|| {
        if input == "-" {
            "plan.png".into()
        } else {
            let p = std::path::Path::new(&input);
            p.with_extension("png").to_string_lossy().into_owned()
        }
    });

    let (plan, ps) = patex::Plan::parse(&src);
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
    Ok(())
}
