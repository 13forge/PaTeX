//! The examples render, check and fix to exactly the checked-in files.

use patex::{check, encode_png, fix, generate, render, GenOptions, Grid, Options, Palette, Plan};

const PLAN: &str = include_str!("../examples/two_bedroom.txt");

#[test]
fn two_bedroom_matches_expected_png() {
    let (plan, ps) = Plan::parse(PLAN);
    assert_eq!((plan.cols(), plan.rows(), ps.unbound), (31, 15, 0));
    let (img, _) = render(&plan, &Options::default());
    assert_eq!(
        encode_png(&img),
        include_bytes!("../examples/two_bedroom.png")
    );
}

#[test]
fn two_bedroom_blueprint_matches_expected_png() {
    let (plan, _) = Plan::parse(PLAN);
    let opt = Options {
        tile_w: 24,
        elev: 12,
        palette: Palette::BLUEPRINT,
        ..Options::default()
    };
    let (img, _) = render(&plan, &opt);
    assert_eq!(
        encode_png(&img),
        include_bytes!("../examples/two_bedroom_blueprint.png")
    );
}

#[test]
fn two_bedroom_is_valid_and_fix_leaves_it_alone() {
    let mut g = Grid::parse(PLAN);
    let rep = check(&g);
    assert!(rep.is_valid(), "{:?}", rep.issues);
    assert_eq!(rep.rooms, 4);
    assert_eq!(fix(&mut g), Default::default());
    assert_eq!(g.to_text(), PLAN);
}

#[test]
fn broken_plan_is_caught_and_fixed_to_the_expected_text() {
    let mut g = Grid::parse(include_str!("../examples/broken.txt"));
    assert_eq!(check(&g).issues.len(), 9);
    fix(&mut g);
    assert!(check(&g).is_valid());
    assert_eq!(g.to_text(), include_str!("../examples/broken_fixed.txt"));
}

#[test]
fn gen_seed7_matches_expected_text() {
    let g = generate(&GenOptions {
        rooms: 5,
        seed: 7,
        ..GenOptions::default()
    });
    assert_eq!(g.to_text(), include_str!("../examples/gen_seed7.txt"));
}

#[test]
fn ptex_wrapper_reads_only_the_topology_body() {
    let src = "\\begin{patex}\n\\chamber{X}\n\\topology{\n┌─┐\n│.│\n└─┘\n}\n\\end{patex}\n";
    let (plan, ps) = Plan::parse(src);
    assert_eq!((plan.cols(), plan.rows(), ps.bound), (3, 3, 9));
}

#[test]
fn labels_and_spaces_are_not_drawn() {
    let (plan, ps) = Plan::parse("│BED 1│");
    assert_eq!(ps.unbound, 4);
    assert!(plan.cell(1, 0).is_none() && plan.cell(0, 0).is_some());
}
