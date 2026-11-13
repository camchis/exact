// A `text` inside a paragraph is an inline box whose padding, border and
// margin take room in the line beside its text (the kernel's
// `TextRun::edges`, LLP 1001; expo/exact#389), as a `<span>`'s do.
import Foundation
import CoreText

extension TextEngine {
    /// The room inline boxes take beside their text (`Run.inlineStart`,
    /// `inlineEnd`), as Core Text's kern after a glyph: a run's end after its
    /// last character, its start after the character before it. The first
    /// run's start has none before it and is the first line's indent
    /// (`Spec.firstLineIndent`). A start at a line's break stays on the
    /// line before it, where CSS puts it at the next line's start.
    package static func kernInlineEdges(_ s: NSMutableAttributedString, runs: [Run]) {
        guard runs.contains(where: { $0.inlineStart != 0 || $0.inlineEnd != 0 }) else { return }
        let text = s.string as NSString
        func add(_ room: CGFloat, before end: Int) {
            guard room != 0, end > 0 else { return }
            let range = text.rangeOfComposedCharacterSequence(at: end - 1)
            let kern = (s.attribute(.kern, at: range.location, effectiveRange: nil) as? CGFloat) ?? 0
            s.addAttribute(.kern, value: kern + room, range: range)
        }
        var offset = 0
        for r in runs {
            let length = r.text.utf16.count
            add(r.inlineStart, before: offset)
            add(r.inlineEnd, before: offset + length)
            offset += length
        }
    }
}
