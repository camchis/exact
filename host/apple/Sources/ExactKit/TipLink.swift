// @ref LLP 1035.006.000 — a tip over an element is a linked capability: the
// `ExactTips` module presents it as TipKit's popover, and an app's
// composition links that module, with TipKit and SwiftUI, only when its plan
// binds `tip`. Without it no tip shows, as on a host with no tip system.
// The core says which tips its elements carry and where each would show:
// over the bar or toolbar item that stands for the element, else over the
// element itself, and nowhere while it is off screen or covered.
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif

/// A tip as its element says it (D1, D2).
package struct TipContent: Equatable {
    /// The element's `id`, which TipKit remembers the tip by.
    package var id: String
    package var title: String
    package var message: String?
    /// The SF Symbols name of its `tipImage`.
    package var symbol: String?
    /// `tipDone`: the person has done what it teaches.
    package var done: Bool
}

/// Where a tip shows now (D3).
package enum TipAnchor {
    #if canImport(UIKit)
    /// The element's view, presented from the controller it is in.
    case view(UIView, from: UIViewController)
    /// The navigation bar item UIKit draws for the element.
    case item(UIBarButtonItem, from: UIViewController)
    #else
    case view(NSView)
    /// The window toolbar's item for the element.
    case item(NSToolbarItem)
    #endif
}

/// What the core asks of the Tips capability.
package protocol TipCapability: AnyObject {
    /// The tips `owner`'s elements carry after a batch, each with where it
    /// would show (nil: nowhere now). A tip `owner` no longer carries is
    /// withdrawn. `store` is where TipKit keeps what it remembers, nil for
    /// its own place; the first update's is used.
    func update(_ tips: [(content: TipContent, anchor: TipAnchor?)], owner: AnyObject, store: URL?)
    /// The agent's `state.tips`: each tip `owner` carries, with what TipKit
    /// says of it and whether it shows.
    func state(owner: AnyObject) -> [[String: Any]]
}

/// The Tips module, installed by the composition.
package enum TipLink {
    package static var installed: TipCapability?
}

extension Presenter {
    /// Hand the capability this presenter's tips: after a batch, once its
    /// bars are projected.
    func presentTips() {
        guard let capability = TipLink.installed else { return }
        let tips: [(content: TipContent, anchor: TipAnchor?)] = carrying("tip").compactMap { node in
            guard let title = node.props["tip"], !title.isEmpty, let id = node.props["id"], !id.isEmpty else { return nil }
            let content = TipContent(id: id, title: title, message: node.props["tipMessage"].flatMap { $0.isEmpty ? nil : $0 },
                                     symbol: node.props["tipSymbolName"].flatMap { $0.isEmpty ? nil : $0 }, done: node.props["tipDone"] == "true")
            return (content, tipAnchor(node))
        }
        capability.update(tips, owner: self, store: tipStore())
    }

    /// Where TipKit keeps what it remembers: its own place, but under the
    /// agent the drive's store, so a drive starts from what that store
    /// holds and `--reset-storage` forgets the tips too.
    private func tipStore() -> URL? {
        guard ExactEnv.agentMode, let session else { return nil }
        let data = URL(fileURLWithPath: NativeViews.roots(session: session).data, isDirectory: true)
        return data.deletingLastPathComponent().appendingPathComponent("tips", isDirectory: true)
    }

    /// Whether `view` and everything above it shows, in a window.
    private func tipShows(_ view: TipView) -> Bool {
        guard let window = view.window else { return false }
        var at: TipView? = view
        while let v = at { if v.isHidden || v.alphaValue <= 0 { return false }; at = v.superview }
        return view.convert(view.bounds, to: nil).intersects(window.contentBounds)
    }

    #if canImport(UIKit)
    private func tipAnchor(_ node: NodeView) -> TipAnchor? {
        if let (item, controller) = navigation.tipItem(node) { return .item(item, from: controller) }
        guard tipShows(node) else { return nil }
        var responder: UIResponder? = node
        while let r = responder, !(r is UIViewController) { responder = r.next }
        // A route a push has covered is still in the window while the push moves.
        guard let controller = responder as? UIViewController,
              controller.navigationController.map({ $0.topViewController === controller }) ?? true else { return nil }
        return .view(node, from: controller)
    }
    #else
    private func tipAnchor(_ node: NodeView) -> TipAnchor? {
        if let item = toolbar.tipItem(node) { return .item(item) }
        return tipShows(node) ? .view(node) : nil
    }
    #endif
}

#if canImport(UIKit)
private typealias TipView = UIView
private extension UIView { var alphaValue: CGFloat { alpha } }
private extension UIWindow { var contentBounds: CGRect { bounds } }

extension NavigationHost {
    /// The navigation bar item a shown route's bar draws for `node`, and
    /// the route it is in; nil when no shown bar stands for it.
    func tipItem(_ node: NodeView) -> (UIBarButtonItem, UIViewController)? {
        for c in controllers.values {
            guard let nav = c.navigationController, nav.topViewController === c, barShows(nav), c.viewIfLoaded?.window != nil,
                  let item = c.barItemsMade[node.id]?.bar else { continue }
            let shown = (c.navigationItem.leftBarButtonItems ?? []) + (c.navigationItem.rightBarButtonItems ?? [])
            if shown.contains(where: { $0 === item }) { return (item, c) }
        }
        return nil
    }
}
#else
private typealias TipView = NSView
private extension NSWindow { var contentBounds: CGRect { contentView?.bounds ?? .zero } }

extension WindowToolbarHost {
    /// The item the window's toolbar shows for `node`; nil when it shows none.
    func tipItem(_ node: NodeView) -> NSToolbarItem? {
        guard let toolbar, let item = items[node.id], toolbar.isVisible, window?.toolbar === toolbar,
              toolbar.items.contains(where: { $0 === item }) else { return nil }
        return item
    }
}
#endif
