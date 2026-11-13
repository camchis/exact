// A `text` inside a paragraph is an inline box whose inline-axis padding,
// border and margin take room in the line (the kernel's `TextRun::edges`):
// on Core Text a kern after the glyph before it, as a `<span>`'s padding
// takes room in Chrome (#389: a verse number touched the next word).
import XCTest
import CoreText
import CExact
@testable import ExactKit

final class InlineEdgesTests: XCTestCase {
    private let engine = TextEngine(resolve: { _ in nil })
    private func run(_ text: String, start: CGFloat = 0, end: CGFloat = 0) -> Run {
        var r = Run(text: text, size: 16, weight: 400, family: 0, italic: false, lineHeight: nil, letterSpacing: 0)
        r.inlineStart = start; r.inlineEnd = end
        return r
    }
    private func spec(_ runs: [Run]) -> Spec {
        Spec(runs: runs, align: 0, lineClamp: 0, color: [0, 0, 0, 255], strut: run(""))
    }
    /// Where the glyph of UTF-16 `index` is drawn on the first line.
    private func x(_ p: Paragraph, _ index: Int) -> CGFloat {
        for run in CTLineGetGlyphRuns(p.lines[0]) as! [CTRun] {
            let count = CTRunGetGlyphCount(run)
            var indices = [CFIndex](repeating: 0, count: count), positions = [CGPoint](repeating: .zero, count: count)
            CTRunGetStringIndices(run, CFRange(location: 0, length: count), &indices)
            CTRunGetPositions(run, CFRange(location: 0, length: count), &positions)
            if let i = indices.firstIndex(of: index) { return positions[i].x }
        }
        return .nan
    }

    func testARunsEdgesTakeRoomBesideItsText() throws {
        let plain = engine.paragraph(spec([run("1"), run("The")]), width: .infinity)
        // A verse number's `padding-right`: the gap after "1", before "The".
        let padded = engine.paragraph(spec([run("1", end: 4.8), run("The")]), width: .infinity)
        XCTAssertEqual(padded.width, plain.width + 4.8, accuracy: 0.05)
        XCTAssertEqual(x(padded, 1), x(plain, 1) + 4.8, accuracy: 0.05)
        // A later run's start goes after the glyph before it.
        let started = engine.paragraph(spec([run("1"), run("The", start: 3)]), width: .infinity)
        XCTAssertEqual(x(started, 1), x(plain, 1) + 3, accuracy: 0.05)
        // The first run's start, with nothing before it, is the first line's.
        let first = engine.paragraph(spec([run("1", start: 6), run("The")]), width: .infinity)
        XCTAssertEqual(first.width, plain.width + 6, accuracy: 0.05)
        XCTAssertEqual(spec([run("1", start: 6)]).firstLineIndent, 6)
    }

    /// The kernel's request carries the edges, and the paragraph it measures
    /// is the one painted from the same runs.
    func testTheRequestsEdgesAreThePaintedSpecs() {
        let texts = ["1", "The"].map { Array($0.utf8) }
        let spec = texts[0].withUnsafeBufferPointer { a in
            texts[1].withUnsafeBufferPointer { b in
                var runs = [a, b].map { bytes -> ExactTextRun in
                    var r = ExactTextRun()
                    r.text = bytes.baseAddress; r.len = bytes.count
                    r.font_size = 16; r.font_weight = 400; r.font_stretch = 100
                    return r
                }
                runs[0].inline_end = 4.8
                var strut = ExactTextRun()
                strut.font_size = 16; strut.font_weight = 400; strut.font_stretch = 100
                return runs.withUnsafeBufferPointer { buffer in
                    var request = ExactMeasureRequest()
                    request.runs = buffer.baseAddress; request.count = buffer.count; request.strut = strut
                    request.width = -1; request.height = -1
                    return engine.requestSpec(request)
                }
            }
        }
        XCTAssertEqual(spec.runs.map(\.inlineEnd), [CGFloat(Float(4.8)), 0])
    }
}
