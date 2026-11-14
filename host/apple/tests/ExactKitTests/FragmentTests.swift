import XCTest
@testable import ExactKit
#if os(macOS)
import AppKit
#else
import UIKit
#endif

final class FragmentTests: XCTestCase {
    private var session: ExactSession!
    private var assets: URL!
    private var oldAssets: URL!
    #if os(macOS)
    private var window: NSWindow!
    /// A field beside the session's view in its window: another owner's focus.
    private var outside: NSTextField?
    #else
    private var window: UIWindow!
    #endif

    override func tearDown() {
        session?.destroy(); session = nil
        #if os(macOS)
        window?.close(); outside = nil
        KeyboardNavigation.agent = nil
        #else
        window?.isHidden = true
        #endif
        window = nil
        if let oldAssets { ExactApp.shared.assetRoot = oldAssets }
        if let assets { try? FileManager.default.removeItem(at: assets) }
        super.tearDown()
    }

    private func fixture() throws {
        #if os(macOS)
        _ = NSApplication.shared
        KeyboardNavigation.agent = true
        #endif
        assets = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: assets, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: assets.appendingPathComponent("assets"), withIntermediateDirectories: true)
        try "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"100\" height=\"800\"><rect width=\"100\" height=\"800\"/></svg>"
            .write(to: assets.appendingPathComponent("assets/late.svg"), atomically: true, encoding: .utf8)
        oldAssets = ExactApp.shared.assetRoot
        ExactApp.shared.assetRoot = assets
        session = ExactApp.shared.makeSession(label: "fragment")
        XCTAssertTrue(session.openIncoming("/item/7#moves"))
        // Keep the real loader pending until the assertion releases the decode.
        session.rasters.testRequest = { _ in (0, 8) }
        let bytes = try Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_FRAGMENT_PLAN"])))
        let size = CGSize(width: 320, height: 400)
        let view = session.presenter.viewport
        view.frame = CGRect(origin: .zero, size: size)
        #if os(macOS)
        window = NSWindow(contentRect: CGRect(origin: .zero, size: size), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.autorecalculatesKeyViewLoop = false
        if let outside {
            let content = NSView(frame: CGRect(origin: .zero, size: size))
            content.addSubview(view); content.addSubview(outside)
            window.contentView = content
        } else {
            window.contentView = view
        }
        #else
        window = UIWindow(frame: CGRect(origin: .zero, size: size))
        let controller = UIViewController()
        controller.view = view
        window.rootViewController = controller
        #endif
        #if os(macOS)
        window.orderFront(nil)
        #else
        window.makeKeyAndVisible()
        #endif
        XCTAssertNil(session.boot(plan: bytes, size: size).error)
        session.apply(session.runtime.dataReady())
        waitFor("the launch fragment") { self.session.presenter.fragmentArm == nil && self.offset > 100 }
        XCTAssertGreaterThan(gap("moves"), 1, "the unloaded tail leaves too little scroll range to align the target")
        XCTAssertNil(try node("picture").raster)
    }

    private func node(_ name: String) throws -> NodeView {
        try XCTUnwrap(session.presenter.views.values.first { $0.props["id"] == name || $0.props["testId"] == name })
    }

    private var offset: CGFloat {
        let port = try! node("port")
        #if os(macOS)
        return port.scroll!.contentView.bounds.minY
        #else
        return port.scrollView!.contentOffset.y
        #endif
    }

    private func gap(_ name: String) -> CGFloat {
        let target = try! node(name), port = try! node("port")
        #if os(macOS)
        return target.convert(target.bounds, to: port.scroll!.contentView).minY - offset
        #else
        return target.convert(target.bounds, to: port.scrollView!).minY - offset
        #endif
    }

    private func releasePicture() throws {
        session.rasters.testRequest = nil
        session.rasters.reconcile()
        waitFor("the picture and its layout") {
            let raster = self.session.rasters.diagnostics
            return (try? self.node("picture").raster) != nil && (try? self.node("tail").raster) != nil
                && raster["pending"] as? UInt64 == 0 && raster["sessionRunning"] as? UInt64 == 0
                && raster["ready"] as? UInt64 == 0
                && self.session.rasters.loadingOnScreen == 0 && !self.session.ownerInFlight
        }
        var idle = false
        session.whenIdle { idle = true }
        waitFor("the image layout's finalization") { idle }
        waitForMainQueue()
    }

    func testFragmentStaysAlignedUntilItsPicturesLand() throws {
        try fixture()
        let before = offset
        try releasePicture()
        XCTAssertGreaterThan(offset, before + 100, "the picture's natural height moves the target")
        XCTAssertEqual(gap("moves"), 0, accuracy: 1)
        XCTAssertNil(session.presenter.fragmentHeld, "finished pictures release the alignment")
    }

    func testFailedPicturesReleaseTheAlignment() throws {
        try fixture()
        try node("picture").loadImage("assets/missing.svg")
        try node("tail").loadImage("assets/missing.svg")
        waitFor("both image failures") { self.session.rasters.testAnnounced.filter { $0.failure != nil }.count == 2 }
        waitForMainQueue()
        XCTAssertNil(session.presenter.fragmentHeld, "an error with no authored handler also ends the hold")
    }

    func testFragmentResumesAfterAnUnchangedFill() throws {
        try fixture()
        try releasePicture()
        let p = session.presenter
        p.scrollElementIntoView(["other", "start", "nearest", "instant"])
        let chosen = offset
        let visit = try XCTUnwrap(Int(try XCTUnwrap(node("port").props["navigationKey"])))
        session.holdFillForTests()
        p.scrollToFragment([visit, "moves", 2])
        waitForMainQueue("the fragment's retry while a fill is pending")
        XCTAssertNotNil(p.fragmentHeld)
        XCTAssertEqual(offset, chosen, accuracy: 1)
        let applied = session.appliedBatches
        session.publishFillForTests(try node("port").id, wireBatch([]), generation: session.generation)
        session.landFill()
        XCTAssertFalse(session.fillInFlight)
        XCTAssertEqual(session.appliedBatches, applied, "an unchanged fill skips the presenter apply pass")
        waitForMainQueue("the fill completion's queued work")
        XCTAssertEqual(gap("moves"), 0, accuracy: 1, "the fill completion resumes the fragment without another batch")
    }

    func testPersonCancelsThePictureHold() throws {
        try fixture()
        let p = session.presenter
        #if os(macOS)
        let port = try XCTUnwrap(node("port").scroll)
        port.contentView.scroll(to: CGPoint(x: 0, y: offset - 120))
        try node("port").liveScrollStarted(Notification(name: NSScrollView.willStartLiveScrollNotification, object: port))
        #else
        let port = try XCTUnwrap(node("port").scrollView)
        p.elementScrolls.userScrolled(port)
        port.setContentOffset(CGPoint(x: 0, y: offset - 120), animated: false)
        #endif
        try releasePicture()
        XCTAssertGreaterThan(abs(gap("moves")), 100, "the person keeps the place they chose")
    }

    func testNavigationWithoutAFragmentCancelsThePictureHold() throws {
        try fixture()
        XCTAssertTrue(session.navigate("/item/7"))
        let chosen = offset
        try releasePicture()
        XCTAssertEqual(offset, chosen, accuracy: 1)
    }

    func testANewerFragmentReplacesThePictureHold() throws {
        try fixture()
        XCTAssertTrue(session.navigate("/item/7#other"))
        XCTAssertEqual(gap("other"), 0, accuracy: 1)
        try releasePicture()
        XCTAssertEqual(gap("other"), 0, accuracy: 1)
    }

    func testAuthoredScrollCancelsThePictureHold() throws {
        try fixture()
        let p = session.presenter
        p.scrollElementIntoView(["other", "start", "nearest", "instant"])
        let chosen = offset
        try releasePicture()
        XCTAssertEqual(offset, chosen, accuracy: 1)
    }

    func testAuthoredOffsetCancelsThePictureHold() throws {
        try fixture()
        session.presenter.apply(wireBatch([["op": "props", "id": try node("port").id, "set": ["scrollTop": "0"]]]))
        XCTAssertEqual(offset, 0, accuracy: 1)
        try releasePicture()
        XCTAssertEqual(offset, 0, accuracy: 1)
    }

    #if os(macOS)
    private func pageScrollCancelsThePictureHold(wheel: Bool) throws {
        try fixture()
        let p = session.presenter, page = p.viewport
        page.documentView?.setFrameSize(CGSize(width: 320, height: 1600))
        page.contentView.scroll(to: CGPoint(x: 0, y: 300))
        if wheel {
            let cg = try XCTUnwrap(CGEvent(scrollWheelEvent2Source: nil, units: .pixel, wheelCount: 1,
                                          wheel1: 80, wheel2: 0, wheel3: 0))
            page.scrollWheel(with: try XCTUnwrap(NSEvent(cgEvent: cg)))
        } else {
            NotificationCenter.default.post(name: NSScrollView.willStartLiveScrollNotification, object: page)
            page.contentView.scroll(to: CGPoint(x: 0, y: 200))
        }
        p.flushFragment()
        XCTAssertNil(p.fragmentHeld, "taking the page scroller cancels the fragment")
        try releasePicture()
        XCTAssertGreaterThan(abs(gap("moves")), 100, "the picture cannot pull the page back to the fragment")
    }

    func testPageWheelCancelsThePictureHold() throws {
        try pageScrollCancelsThePictureHold(wheel: true)
    }

    func testPageLiveScrollCancelsThePictureHold() throws {
        try pageScrollCancelsThePictureHold(wheel: false)
    }

    private func tab(back: Bool = false) -> Bool {
        let key = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: back ? .shift : [], timestamp: 0,
                                   windowNumber: window.windowNumber, context: nil, characters: "\t",
                                   charactersIgnoringModifiers: "\t", isARepeat: false, keyCode: 48)!
        return session.presenter.routeKey(key, focused: true, in: window)
    }

    func testTabBeginsAtTheFragmentTarget() throws {
        try fixture()
        let p = session.presenter
        XCTAssertTrue(tab())
        XCTAssertEqual(p.focusedNode?.props["id"], "after")
        XCTAssertTrue(session.navigate("/item/7#moves"))
        XCTAssertTrue(tab(back: true))
        XCTAssertEqual(p.focusedNode?.props["id"], "before")
    }

    func testAFragmentLeavesFocusOutsideItsSessionAlone() throws {
        outside = NSTextField(frame: CGRect(x: 0, y: 0, width: 100, height: 24))
        try fixture()
        XCTAssertTrue(window.makeFirstResponder(outside))
        let editor = try XCTUnwrap(outside?.currentEditor())
        XCTAssertTrue(session.navigate("/item/7#moves"))
        waitForMainQueue()
        XCTAssertTrue(window.firstResponder === editor, "the other owner's editing goes on")
    }

    func testANavigationFromABlurHandlerKeepsItsTabStart() throws {
        try fixture()
        let p = session.presenter
        let leaver = try node("leaver")
        XCTAssertTrue(window.makeFirstResponder(p.keyView(of: leaver)))
        XCTAssertTrue(p.focusedNode === leaver)
        XCTAssertTrue(session.navigate("/item/7#moves"))
        waitForMainQueue()
        XCTAssertEqual(gap("other"), 0, accuracy: 1, "the blur's navigation supersedes")
        XCTAssertTrue(tab())
        XCTAssertEqual(p.focusedNode?.props["id"], "before", "Tab goes on from the superseding fragment")
    }

    func testKeyboardScrollingCancelsThePictureHold() throws {
        try fixture()
        XCTAssertTrue(session.presenter.keyScroll("moveToBeginningOfDocument:", from: try node("moves")))
        XCTAssertEqual(offset, 0, accuracy: 1)
        try releasePicture()
        XCTAssertEqual(offset, 0, accuracy: 1)
    }
    #else
    func testFragmentResumesWhenItsTargetMountsAfterTheQueuedRetry() throws {
        let p = Presenter()
        p.viewport.frame = CGRect(x: 0, y: 0, width: 320, height: 400)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "props": ["navigationKey": "7"], "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 2, "kind": "view", "props": ["id": "moves"]],
            ["op": "create", "id": 3, "kind": "view"],
            ["op": "children", "id": 1, "ids": [2, 3]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 320, "h": 400],
            ["op": "frame", "id": 2, "x": 0, "y": 600, "w": 100, "h": 40],
            ["op": "frame", "id": 3, "x": 0, "y": 1000, "w": 100, "h": 400],
            ["op": "content", "id": 1, "w": 320, "h": 1400],
        ]))
        p.scrollToFragment([7, "moves", 1])
        waitForMainQueue("the unmounted fragment's retry")
        XCTAssertTrue(p.elementScrolls.names.isEmpty)
        XCTAssertNotNil(p.fragmentHeld)
        let scroll = try XCTUnwrap(p.views[1]?.scrollView)
        XCTAssertEqual(scroll.contentSize.height, 1400, accuracy: 1)
        XCTAssertEqual(scroll.contentOffset.y, 0, accuracy: 1)
        window = UIWindow(frame: p.viewport.frame)
        window.addSubview(p.viewport)
        XCTAssertEqual(scroll.contentOffset.y, 600, accuracy: 1, "mounting resumes the fragment without another batch")
        withExtendedLifetime(p) {}
    }
    #endif
}
