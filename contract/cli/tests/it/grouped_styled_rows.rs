//! LLP 1084 D7: a grouped row the platform's standard cell can't draw is its
//! own views. A row holding one `column` of one or two texts is the subtitle
//! cell, which on iOS is a `UIListContentConfiguration` that draws nothing of
//! a box; a row or column whose author styled its box keeps its views (#327).

use exact_kernel::{Accessory, GroupedRow, Kernel, Offer};
use exact_runner::{DataError, DataSource, Runner, Value};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

/// `styles` above a component whose view is `body` under a 402 × 874 column.
fn boot(styles: &str, body: &str) -> Runner<NoData> {
    let body = body
        .lines()
        .map(|l| format!("      {l}\n"))
        .collect::<String>();
    let source = format!(
        "{styles}component App\n  state on = true\n  action flip(v: bool)\n    on = v\n  view\n    column testId=\"root\" width=402 height=874\n{body}"
    );
    let plan = contract::bake(
        contract::compile(&source).unwrap_or_else(|e| panic!("{e}")),
        NoData,
    )
    .unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let k = r.kernel_mut();
    let root = k.node_by_key(k.find_by_test_id("root")[0]).unwrap().id;
    k.compute_layout(root, Offer::definite(402.0, 874.0))
        .unwrap();
    r
}

fn id(r: &Runner<NoData>, test_id: &str) -> u32 {
    let k = r.kernel();
    k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
}

fn read(row: &GroupedRow) -> (bool, bool, Option<String>) {
    (row.custom, row.subtitle, row.title.clone())
}

#[test]
fn a_row_whose_box_or_columns_box_is_styled_keeps_its_views() {
    // A tile in a grouped list (a port's Credits): a column of two texts
    // given a size, a ground, corners and a centre was read as a subtitle
    // cell, and iOS's content configuration drew none of them.
    let r = boot(
        "style Tile\n  height=300\n",
        "list appearance=\"auto\" testId=\"list\" flex=1\n  section\n    row\n      column height=300 flex=1 align-items=\"center\" justify-content=\"center\" border-radius=16 background-color=\"-exact-fill\"\n        text \"Puzzle\"\n        text \"A 300-point tile\"\n    row\n      column min-height=300 flex=1\n        text \"Puzzle\"\n        text \"A 300-point tile\"\n    row height=300\n      column height=\"100%\" flex=1\n        text \"Puzzle\"\n        text \"A 300-point tile\"\n    row height=300\n      column flex=1\n        text \"Puzzle\"\n        text \"Only the row has a height\"\n    row\n      column flex=1 align-items=\"center\"\n        text \"Puzzle\"\n        text \"Centred\"\n    row\n      column class=Tile flex=1\n        text \"Puzzle\"\n        text \"A class's height\"\n    row flex-direction=\"column\" background-color=\"-exact-fill\"\n      text \"Title\"\n      text \"Detail\"",
    );
    let rows = &r.kernel().grouped_list(id(&r, "list")).unwrap().sections[0].rows;
    assert_eq!(rows.len(), 7, "every variant is read");
    for (row, why) in rows.iter().zip([
        "a sized, grounded, rounded, centred column",
        "a column's minimum height",
        "a row's height",
        "a row's height alone",
        "a column's alignment",
        "a class's height on the column",
        "a column row's own ground",
    ]) {
        assert_eq!(read(row), (true, false, None), "{why}");
    }
}

#[test]
fn a_plain_column_of_texts_is_still_the_subtitle_cell() {
    // Beside a switch, and under a row's minimum height, which UIKit keeps.
    let r = boot(
        "",
        "list appearance=\"auto\" testId=\"list\" flex=1\n  section\n    row\n      column flex=1\n        text \"Sound\"\n        text \"Hear what you're touching.\"\n      input type=\"checkbox\" switch checked=on input=flip testId=\"switch\"\n    row min-height=80\n      column\n        text \"Title\"\n        text \"Detail\"",
    );
    let rows = &r.kernel().grouped_list(id(&r, "list")).unwrap().sections[0].rows;
    assert_eq!(read(&rows[0]), (false, true, Some("Sound".into())));
    assert_eq!(rows[0].accessory, Accessory::Toggle(id(&r, "switch")));
    assert_eq!(
        read(&rows[1]),
        (false, true, Some("Title".into())),
        "a row's minimum height is UIKit's to keep"
    );
}
