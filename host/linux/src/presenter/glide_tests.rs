//! A smooth element reveal glides while the display owns the clock, owns
//! its scroller until it lands (the end-follow waits), and ends once.
use super::super::{PainterChoice, Presenter};
use exact_kernel::ViewId;
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Event};
use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;

#[derive(Default)]
struct NoData;
impl DataSource for NoData {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
}

/// Rows 50 tall in a port 100 tall: the end is at `50 n - 100`.
const APP: &str = r#"component App
  state n = 10
  state following = true
  state ends = 0
  action up
    scrollIntoView("row-0", behavior="smooth")
  action down
    following = true
    scrollIntoView(`row-${n - 1}`, block="end", behavior="smooth")
  action jump
    scrollIntoView("row-0", behavior="instant")
  action away
    following = false
  action add
    n = n + 2
  action ended
    ends = ends + 1
  view
    column
      button "Up" press=up testId="up"
      button "Down" press=down testId="down"
      button "Jump" press=jump testId="jump"
      button "Away" press=away testId="away"
      button "Add" press=add testId="add"
      scroll testId="port" width=200 height=100 scrollFollowEnd=following scrollend=ended
        for i in range(n) key=i
          box id=`row-${i}` width=200 height=50 flex-shrink=0
"#;

struct Drive {
    p: Presenter<NoData>,
    wall: Rc<Cell<f64>>,
}

impl Drive {
    fn boot() -> Self {
        let (mut p, error) = Presenter::boot_with(
            &contract::compile(APP).unwrap().encode(),
            NoData,
            (400., 500.),
            1.,
            PathBuf::new(),
            PainterChoice::Cpu,
        )
        .unwrap();
        assert_eq!(error, None);
        let wall = Rc::new(Cell::new(0.0));
        let read = wall.clone();
        p.set_live_clock(Some(Rc::new(move || read.get())));
        let mut d = Drive { p, wall };
        d.frame(0.);
        d
    }
    fn id(&self, name: &str) -> ViewId {
        let k = self.p.host.kernel();
        k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
    }
    fn top(&self) -> f32 {
        self.p.scroll_of(self.id("port")).1
    }
    fn number(&self, name: &str) -> f64 {
        match self.p.host.runner().slot(name).unwrap() {
            Value::Number(n) => *n,
            value => panic!("{name}: {value:?}"),
        }
    }
    fn press(&mut self, name: &str) {
        let (id, at) = (self.id(name), self.wall.get());
        assert_eq!(self.p.host.dispatch_at(id, Event::Press, at), None);
        assert_eq!(self.p.after_commit(), None);
        self.p.run_commands(NoData::default);
    }
    /// A display frame at `at` ms: the motion tick, then the timers.
    fn frame(&mut self, at: f64) {
        self.wall.set(at);
        self.p.tick(at);
        assert_eq!(self.p.advance(at), None);
    }
}

#[test]
fn a_smooth_reveal_glides_on_the_displays_clock_and_ends_once() {
    let mut d = Drive::boot();
    // Following its end, at it.
    assert_eq!(d.top(), 400.);
    d.press("up");
    assert_eq!(d.top(), 400., "it starts from where the port was");
    assert!(d.p.needs_animation_frame());
    d.frame(100.);
    let early = d.top();
    d.frame(150.);
    let middle = d.top();
    assert!(
        early < 400. && middle < early && middle > 0.,
        "{early} {middle}"
    );
    assert_eq!(d.number("ends"), 0.);
    d.frame(300.);
    assert_eq!(d.top(), 0.);
    d.frame(316.);
    assert_eq!(d.number("ends"), 1., "one end, where it rests");
    assert!(!d.p.needs_animation_frame());
    d.frame(500.);
    assert_eq!(d.number("ends"), 1.);
}

#[test]
fn the_end_follow_waits_for_a_glide_and_resumes_only_where_it_rests_at_the_end() {
    let mut d = Drive::boot();
    // Away from a followed end: rows arriving mid-glide are not followed.
    d.press("up");
    d.frame(100.);
    d.press("add");
    let at = d.top();
    assert!(at > 0. && at < 400., "{at}");
    d.frame(200.);
    assert!(d.top() < at, "the glide goes on up");
    d.frame(400.);
    assert_eq!(d.top(), 0.);
    d.press("add");
    assert_eq!(d.top(), 0., "it rests at the start: not following");

    // The app turns following on with a glide to the last row: no jump to
    // the end first, and once it rests there the end is followed.
    d.press("away");
    d.press("down");
    assert_eq!(d.top(), 0., "no jump ahead of the glide");
    d.frame(550.);
    assert!(d.top() > 0. && d.top() < 600., "{}", d.top());
    d.frame(800.);
    assert_eq!(d.top(), 600.);
    d.press("add");
    d.frame(816.);
    assert_eq!(d.top(), 700., "at the end, the end is followed");
}

#[test]
fn another_move_of_the_port_stops_a_glide_where_it_is() {
    let mut d = Drive::boot();
    d.press("up");
    d.frame(100.);
    let port = d.id("port");
    // The person's scroll (a key, a scrollbar) takes the port over.
    assert_eq!(d.p.scroll_to(port, (0., 123.)), Some((0., 123.)));
    d.frame(200.);
    assert_eq!(d.top(), 123.);
    assert!(!d.p.needs_animation_frame());
    // An instant request replaces a glide at once.
    d.press("up");
    d.frame(250.);
    d.press("jump");
    assert_eq!(d.top(), 0.);
    d.frame(600.);
    assert_eq!(d.top(), 0.);
}

#[test]
fn reduced_motion_and_a_clock_the_display_does_not_own_land_at_once() {
    let mut d = Drive::boot();
    d.p.set_live_clock(None);
    d.press("up");
    assert_eq!(d.top(), 0.);
    let mut d = Drive::boot();
    let mut preferences = d.p.host().runner().viewport().preferences;
    preferences.reduced_motion = true;
    d.p.set_preferences(preferences);
    d.press("up");
    assert_eq!(d.top(), 0.);
}
