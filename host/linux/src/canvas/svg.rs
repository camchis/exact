//! An SVG shape's fill and stroke as the reader draws them: `PATH` and
//! `STROKE` ops, a `DASH` before a dashed stroke (`Backend::svg_path`).
use super::{path_bounds, Recorder, DASH, PATH, STROKE};
use crate::paint::border::PathOp;
use crate::paint::SvgPaint;
use tiny_skia::Transform;

impl Recorder {
    pub(super) fn svg_shape(&mut self, s: &SvgPaint<'_>, ts: Transform) {
        let ops: Vec<PathOp> = s
            .path
            .0
            .iter()
            .map(|seg| match *seg {
                exact_kernel::svg::Seg::Move(x, y) => PathOp::Move(x, y),
                exact_kernel::svg::Seg::Line(x, y) => PathOp::Line(x, y),
                exact_kernel::svg::Seg::Cubic(a, b, c, d, x, y) => PathOp::Cubic(a, b, c, d, x, y),
                exact_kernel::svg::Seg::Close => PathOp::Close,
            })
            .collect();
        let grow = if s.stroke.is_some() { s.width } else { 0.0 };
        self.need(path_bounds(&ops).and_then(|(x, y, w, h)| {
            self.on_page((x - grow, y - grow, w + 2.0 * grow, h + 2.0 * grow), ts)
        }));
        self.transform(ts);
        if let Some(c) = s.fill.as_ref().and_then(|ink| self.ink(ink)) {
            self.ops.extend([PATH, c, u32::from(s.even_odd)]);
            self.path(&ops);
        }
        // A gradient's SHADER goes before the DASH, which the reader keeps
        // as it does a shader until the STROKE.
        if let Some(c) = s.stroke.as_ref().and_then(|ink| self.ink(ink)) {
            if !s.dash.is_empty() {
                self.ops.push(DASH);
                // A layer playing the dash offset finds its phase here.
                self.layers.note_dash(self.ops.len());
                self.f(s.phase);
                self.ops.push(s.dash.len() as u32);
                for d in &s.dash {
                    self.f(*d);
                }
            }
            self.ops.extend([STROKE, c]);
            self.f(s.width);
            self.ops.extend([u32::from(s.cap), u32::from(s.join)]);
            self.path(&ops);
        }
    }
}
