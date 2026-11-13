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

    /// Where a line breaks right before a run with a start, the room starts
    /// the next line with its text, as Chrome keeps a `<span>`'s padding with
    /// it; the space before it, kerned, hangs at the end of the line before.
    func testAStartAtABreakStartsTheNextLine() {
        let plain = engine.paragraph(spec([run("aaaa "), run("bbbb")]), width: .infinity)
        let padded = spec([run("aaaa "), run("bbbb", start: 20)])
        let one = engine.paragraph(padded, width: .infinity)
        XCTAssertEqual(one.lines.count, 1)
        XCTAssertEqual(x(one, 5), x(plain, 5) + 20, accuracy: 0.05, "on one line, after the space")
        XCTAssertEqual(one.insets.at(5).width, 20, "a line starting there would start with it")
        let two = engine.paragraph(padded, width: plain.width + 10)
        XCTAssertEqual(two.lines.count, 2)
        XCTAssertEqual(CTLineGetStringRange(two.lines[1]).location, 5)
        XCTAssertEqual(two.insets.at(5).left, 20, "the second line starts at the room")
        // Its width is its wider line's: the second, with the room.
        let word = engine.paragraph(spec([run("bbbb")]), width: .infinity).width
        let first = engine.paragraph(spec([run("aaaa")]), width: .infinity).width
        XCTAssertEqual(two.width, max(first, word + 20), accuracy: 0.5)
        // Min-content: the word carries its room; the space hangs.
        XCTAssertEqual(engine.minContentWidth(padded), word + 20, accuracy: 0.5)
        // Glued to a word before it, with no space, it never starts a line.
        XCTAssertTrue(LineInsets.edgeStarts(spec([run("aaaa"), run("bbbb", start: 20)]).runs).isEmpty)
    }

    /// A box's background covers its padding (CSS 2 §14.2): from its first
    /// glyph back by its start, to its last glyph's end past the room kerned
    /// after it less its margin.
    func testABackgroundCoversItsPadding() throws {
        let gray: [Double] = [242, 242, 247, 255]
        func boxed(_ text: String, start: CGFloat, end: CGFloat, pad: (CGFloat, CGFloat)) -> Run {
            var r = run(text, start: start, end: end)
            r.background = gray; r.backgroundBox = 9; r.backgroundBefore = pad.0; r.backgroundAfter = pad.1
            return r
        }
        func fill(_ runs: [Run]) throws -> CGRect {
            let p = engine.paragraph(spec(runs), width: .infinity)
            return try XCTUnwrap(TextLinePaint.backgrounds(p.lines[0], at: .zero).first?.rect)
        }
        let bare = try fill([run("a "), boxed("code", start: 0, end: 0, pad: (0, 0)), run(" b")])
        // 4 of padding each side, and 3 of margin after it, which the background leaves out.
        let padded = try fill([run("a "), boxed("code", start: 4, end: 7, pad: (4, 4)), run(" b")])
        XCTAssertEqual(padded.minX, bare.minX, accuracy: 0.05, "the room starts where the bare box did")
        XCTAssertEqual(padded.width, bare.width + 8, accuracy: 0.05)
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
