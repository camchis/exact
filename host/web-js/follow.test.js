// `scrollFollowEnd` on an ordinary scroller on the JS target (follow.js; LLP 1070.000 §15, expo/exact#370), driven
// through the runtime's hooks as a commit runs them: `Before`, the props, `After`, then the commit's commands, the
// follow settling once they have run. Stand-in elements and ResizeObserver; the follower is the web host's own.
import { test, expect, mock } from "bun:test";

mock.module("./rt.js", () => ({ After: [], Before: [], Hosts: {}, PropHooks: {}, clock: { agent: false } }));
mock.module("./navigation.js", () => import("../web/navigation.js"));
globalThis.ResizeObserver = class { constructor(fn) { this.fn = fn; } observe() {} disconnect() {} };
const { endFollow } = await import("./follow.js");
const { scrollFollowers } = await import("../web/navigation.js");

/** A scroller `port` tall over `height` of content at `top`, holding the element `target`, under a fresh runtime. */
function setup({ height = 1000, port = 100, top = 900, smooth = false } = {}) {
  const heard = {}, target = {};
  const el = {
    scrollTop: top, scrollHeight: height, clientHeight: port, isConnected: true, children: [], style: { scrollBehavior: smooth ? "smooth" : "" },
    contains: (e) => e === target,
    addEventListener: (type, fn) => { heard[type] = fn; },
    removeEventListener: (type) => { delete heard[type]; },
  };
  const revealed = [];
  const rt = { After: [], Before: [], Hosts: { scrollIntoView: (...a) => revealed.push(a) }, PropHooks: {}, clock: { agent: false } };
  const { F } = endFollow(rt, { scrollFollowers, liveById: (id) => (id === "target" ? target : null) }, (e) => e.style);
  /** One commit: its props, then its commands; resolves once the follow has settled. */
  const commit = async ({ props = {}, grow = 0, commands = [] } = {}) => {
    for (const f of rt.Before) f();
    el.scrollHeight += grow;
    for (const [name, v] of Object.entries(props)) {
      if (!rt.PropHooks[name]?.(el, v) && name === "scrollTop") el.scrollTop = Number(v); // rt.js `drain`, instant
    }
    for (const f of rt.After) f();
    for (const c of commands) rt.Hosts.scrollIntoView(...c);
    await Promise.resolve();
  };
  const scroll = (to) => { el.scrollTop = to; heard.scroll(); };
  return { el, F, commit, scroll, heard, revealed };
}

test("a follower at its end follows content that grows; one away from it does not", async () => {
  const s = setup();
  await s.commit({ props: { scrollFollowEnd: "true" } });
  await s.commit({ grow: 100 });
  expect(s.el.scrollTop).toBe(1000);
  s.scroll(400);
  await s.commit({ grow: 100 });
  expect(s.el.scrollTop).toBe(400);
});

test("turning the follow on with a smooth scrollIntoView does not jump to the end first", async () => {
  const s = setup({ top: 200 });
  await s.commit({ props: { scrollFollowEnd: "true" }, commands: [["target", "end", "nearest", "smooth"]] });
  expect(s.revealed.length).toBe(1);
  expect(s.el.scrollTop).toBe(200);
  // Mid-flight, content arrives: the request's, not the follow's.
  s.scroll(500);
  await s.commit({ grow: 50 });
  expect(s.el.scrollTop).toBe(500);
  // It rests at the end: from there the end is followed.
  s.scroll(950);
  s.heard.scrollend();
  await s.commit({ grow: 100 });
  expect(s.el.scrollTop).toBe(1050);
});

test("an instant scrollTop write in the commit that turns the follow on is where the port rests", async () => {
  const s = setup({ top: 200 });
  await s.commit({ props: { scrollFollowEnd: "true", scrollTop: "300" } });
  expect(s.el.scrollTop).toBe(300);
  await s.commit({ grow: 100 });
  expect(s.el.scrollTop).toBe(300);
});

test("a virtualized list's follow is list.js's", async () => {
  const s = setup();
  s.el.$list = {};
  await s.commit({ props: { scrollFollowEnd: "true" } });
  expect(s.F.followedScrolls.size).toBe(0);
});
