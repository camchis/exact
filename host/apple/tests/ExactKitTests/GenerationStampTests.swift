import Foundation
import XCTest
@testable import ExactKit

/// A platform answer reaches only the session generation that asked
/// (`GenerationStamp`): a reload keeps the session but starts a runner that
/// numbers its calls and nodes from the start again.
final class GenerationStampTests: XCTestCase {
    private var sessions: [ExactSession] = []
    override func tearDown() { sessions.forEach { $0.destroy() }; sessions = []; super.tearDown() }

    private func compile(_ source: String) throws -> Data {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        try source.write(to: dir.appendingPathComponent("app.contract"), atomically: true, encoding: .utf8)
        let compiler = Process()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"], "build.mjs --test builds the Contract compiler and names it"))
        compiler.arguments = ["build", dir.appendingPathComponent("app.contract").path, "-o", dir.appendingPathComponent("app.plan").path]
        compiler.standardError = FileHandle.nullDevice
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0)
        return try Data(contentsOf: dir.appendingPathComponent("app.plan"))
    }

    private static let plain = """
    component Probe
      view
        column testId="page"
          text "page"

    """

    /// A session running `source`, with no view.
    private func session(_ source: String = plain) throws -> ExactSession {
        #if os(macOS)
        _ = NSApplication.shared
        #endif
        let session = ExactApp.shared.makeSession(label: "stamp-\(UUID().uuidString.prefix(6))")
        sessions.append(session)
        session.presenter.viewport.frame = CGRect(x: 0, y: 0, width: 300, height: 400) // what a reload lays out in
        XCTAssertNil(session.boot(plan: try compile(source), size: CGSize(width: 300, height: 400)).error)
        return session
    }

    private func logs(_ session: ExactSession) -> String { session.agent(#"{"op":"logs","since":0}"#) }

    /// Drain what the main queue holds now.
    private func drain() { RunLoop.main.run(until: Date().addingTimeInterval(0.05)) }

    func testAStampOutlivesNeitherItsGenerationNorItsSession() throws {
        let session = try session()
        let stamp = GenerationStamp(session)
        XCTAssertTrue(stamp.session === session)
        XCTAssertTrue(session.apply(try compile(Self.plain)), "reloaded")
        XCTAssertNil(stamp.session, "a reload ends the generation")
        let next = GenerationStamp(session)
        XCTAssertTrue(next.session === session)
        session.destroy()
        XCTAssertNil(next.session, "so does the session's end")
    }

    // MARK: presence

    /// The check's ending, here synchronous: this test host's module serves no
    /// authenticator, so the check fails at once and journals why.
    private let presence: [String: Any] = ["call": 0, "name": "token", "policy": "user-presence"]
    private let presenceFailed = "presence token: the platform's check failed"

    func testAPresenceAnswerReachesTheGenerationThatAsked() throws {
        let session = try session()
        session.presenceOp(presence, issued: GenerationStamp(session))
        drain()
        XCTAssertTrue(logs(session).contains(presenceFailed), logs(session))
    }

    func testAPresenceAnswerAfterAReloadReachesNoCall() throws {
        let session = try session()
        let asked = GenerationStamp(session)
        XCTAssertTrue(session.apply(try compile(Self.plain)))
        session.presenceOp(presence, issued: asked)
        drain()
        XCTAssertFalse(logs(session).contains(presenceFailed), "the old check answers the new runner's call 0")
    }

    func testAPresenceOpQueuedWhenItsGenerationEndsIsNotStarted() throws {
        let session = try session()
        session.apply(wireBatch([["op": "presence"].merging(presence) { a, _ in a }]))
        drain()
        XCTAssertTrue(logs(session).contains(presenceFailed), "the batch's check started: \(logs(session))")
        session.apply(wireBatch([["op": "presence", "call": 1, "name": "key", "policy": "user-presence"]]))
        // A reload pumps the main queue while it prepares, so the generation ends here with nothing run in between.
        session.commitForTests()
        drain()
        XCTAssertFalse(logs(session).contains("presence key:"), "the old batch's check was started")
    }
}
