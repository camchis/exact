// `scrollFollowEnd` on an ordinary scroller on the JS target, imported by an app's module only when its plan has one
// (exact-web-js `emit.rs`); a virtualized list's is list.js's. The wasm host's follower (navigation.js
// `scrollFollowers`) keeps a scroller at its end as its content and port resize, while the reader is there. A scroll
// the app makes (`scrollIntoView`, a `scrollTop` write) owns the scrollers it moves until it settles (LLP 1070.000
// §15): the follow moves nothing meanwhile, `scrollFollowEnd` turning true in the same commit included, and where
// the scroll rests decides whether the end is followed. A commit's follow settles once its commands have run.
import { After, Before, Hosts, PropHooks, clock } from "./rt.js";
import { liveById, scrollFollowers } from "./navigation.js";

/** Installs the follow over the runtime's hooks; returns the follower table and the commit's settle, for tests. */
export function endFollow(rt, nav, styleOf = (e) => getComputedStyle(e)) {
  const F = nav.scrollFollowers(() => {});
  // This commit's authored scrolls: the element moved and its behaviour (`smooth` settles at its `scrollend`).
  let authored = [], queued = false;
  const settle = () => {
    queued = false;
    for (const [e, behavior] of authored.splice(0)) {
      for (const s of F.own(e)) {
        if (behavior === "instant" || (behavior === "auto" && styleOf(s.el).scrollBehavior !== "smooth")) F.release(s);
      }
    }
    for (const s of F.followedScrolls.values()) {
      if (s.el.$list) F.followScroll(s.el, false);
      else F.settleFollow(s);
    }
  };
  rt.Before.push(() => { for (const s of F.followedScrolls.values()) s.scrolled(); });
  const queue = () => { if (!queued) { queued = true; queueMicrotask(settle); } };
  rt.After.push(queue);
  rt.PropHooks.scrollFollowEnd = (e, v) => {
    if (e.$list) return false;
    F.followScroll(e, v === "true");
    return true;
  };
  for (const name of ["scrollTop", "scrollLeft"]) {
    const prior = rt.PropHooks[name];
    rt.PropHooks[name] = (e, v) => {
      // rt.js `drain` animates it under `scroll-behavior: smooth`, except on the agent's clock.
      if (v != null) authored.push([e, !rt.clock.agent && e.style?.scrollBehavior === "smooth" ? "smooth" : "instant"]);
      return prior?.(e, v) ?? false;
    };
  }
  const element = rt.Hosts.scrollIntoView;
  rt.Hosts.scrollIntoView = (...a) => {
    const e = a.length !== 7 ? nav.liveById(a[0]) : null;
    if (e) { authored.push([e, a[3] ?? "auto"]); queue(); }
    return element(...a);
  };
  return { F, settle };
}

endFollow({ After, Before, Hosts, PropHooks, clock }, { liveById, scrollFollowers });
