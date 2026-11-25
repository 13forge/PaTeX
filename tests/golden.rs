//! The example plans render to exactly the checked-in PNGs.

use patex::{encode_png, render, Options, Palette, Plan};

const PLAN: &str = include_str!("../examples/two_bedroom.txt");

#[test]
fn two_bedroom_matches_expected_png() {
    let (plan, ps) = Plan::parse(PLAN);
    assert_eq!((plan.cols(), plan.rows(), ps.unbound), (31, 15, 0));
    let (img, _) = render(&plan, &Options::default());
    assert_eq!(encode_png(&img), include_bytes!("../examples/two_bedroom.png"));
}

#[test]
fn two_bedroom_blueprint_matches_expected_png() {
    let (plan, _) = Plan::parse(PLAN);
    let opt = Options { tile_w: 24, elev: 12, palette: Palette::BLUEPRINT, ..Options::default() };
    let (img, _) = render(&plan, &opt);
    assert_eq!(encode_png(&img), include_bytes!("../examples/two_bedroom_blueprint.png"));
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
