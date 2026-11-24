//! Synthetic DWG fixtures, written with the acadrust writer at test time.
//! No drawing is committed to the repository.

use std::path::{Path, PathBuf};

use acadrust::entities::{Circle, EntityType, Line, MText, Text, Viewport};
use acadrust::tables::Layer;
use acadrust::types::{Color, Vector3};
use acadrust::{CadDocument, DwgWriter, DxfVersion};

/// Labels of the `plan` fixture, by layer.
pub const PLAN_WALL_LABELS: [&str; 3] = ["WALL A-101", "WALL A-102", "North facade"];
pub const PLAN_PIPE_LABELS: [&str; 2] = ["PIPE DN100", "PIPE DN150"];
pub const PLAN_MTEXT_LABEL: &str = "General note: all sizes in mm";
pub const PLAN_LABEL_COUNT: usize = 6;

/// DWG stores an entity's layer by handle, so a layer needs one before use.
fn add_layer(doc: &mut CadDocument, name: &str, color: Color) {
    let mut layer = Layer::with_color(name, color);
    layer.handle = doc.allocate_handle();
    doc.layers.add(layer).expect("add layer");
}

fn text_on(layer: &str, value: &str, x: f64, y: f64, height: f64) -> EntityType {
    let mut t = Text::with_value(value, Vector3::new(x, y, 0.0)).with_height(height);
    t.common.layer = layer.to_string();
    EntityType::Text(t)
}

fn line_on(layer: &str, x1: f64, y1: f64, x2: f64, y2: f64) -> EntityType {
    let mut l = Line::from_coords(x1, y1, 0.0, x2, y2, 0.0);
    l.common.layer = layer.to_string();
    EntityType::Line(l)
}

/// A small floor plan: two named layers, geometry, six labels, one paper sheet.
pub fn plan_document() -> CadDocument {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    add_layer(&mut doc, "WALLS", Color::RED);
    add_layer(&mut doc, "PIPES", Color::BLUE);

    for (x1, y1, x2, y2) in [
        (0.0, 0.0, 400.0, 0.0),
        (400.0, 0.0, 400.0, 300.0),
        (400.0, 300.0, 0.0, 300.0),
        (0.0, 300.0, 0.0, 0.0),
    ] {
        doc.add_entity(line_on("WALLS", x1, y1, x2, y2)).expect("wall");
    }
    doc.add_entity(line_on("PIPES", 50.0, 150.0, 350.0, 150.0)).expect("pipe");
    let mut c = Circle::from_coords(200.0, 150.0, 0.0, 40.0);
    c.common.layer = "PIPES".to_string();
    doc.add_entity(EntityType::Circle(c)).expect("circle");

    doc.add_entity(text_on("WALLS", PLAN_WALL_LABELS[0], 20.0, 270.0, 12.0)).expect("text");
    doc.add_entity(text_on("WALLS", PLAN_WALL_LABELS[1], 220.0, 270.0, 12.0)).expect("text");
    doc.add_entity(text_on("WALLS", PLAN_WALL_LABELS[2], 20.0, 20.0, 12.0)).expect("text");
    doc.add_entity(text_on("PIPES", PLAN_PIPE_LABELS[0], 60.0, 160.0, 10.0)).expect("text");
    doc.add_entity(text_on("PIPES", PLAN_PIPE_LABELS[1], 240.0, 160.0, 10.0)).expect("text");
    let mut m = MText::with_value(PLAN_MTEXT_LABEL, Vector3::new(20.0, 60.0, 0.0)).with_height(10.0);
    m.common.layer = "WALLS".to_string();
    doc.add_entity(EntityType::MText(m)).expect("mtext");

    let mut vp = Viewport::new();
    vp.id = 1;
    vp.width = 297.0;
    vp.height = 210.0;
    vp.center = Vector3::new(148.5, 105.0, 0.0);
    doc.add_paper_space_entity(EntityType::Viewport(vp)).expect("viewport");
    doc
}

/// A drawing with geometry and no text at all.
pub fn blank_document() -> CadDocument {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    doc.add_entity(line_on("0", 0.0, 0.0, 100.0, 100.0)).expect("line");
    doc.add_entity(line_on("0", 0.0, 100.0, 100.0, 0.0)).expect("line");
    doc
}

/// A drawing with `n` numbered labels on a grid, for paging tests.
pub fn dense_document(n: usize) -> CadDocument {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    add_layer(&mut doc, "TAGS", Color::GREEN);
    doc.add_entity(line_on("0", 0.0, 0.0, 2000.0, 0.0)).expect("line");
    for i in 0..n {
        let x = (i % 20) as f64 * 100.0;
        let y = (i / 20) as f64 * 40.0 + 20.0;
        doc.add_entity(text_on("TAGS", &format!("TAG-{i:04}"), x, y, 10.0)).expect("tag");
    }
    doc
}

pub fn write_dwg(doc: &CadDocument, dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    DwgWriter::write_to_file(&path, doc).expect("write dwg fixture");
    path
}
