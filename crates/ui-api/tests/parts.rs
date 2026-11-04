//! A context's viewport parts as the host holds them: each part a package
//! gives is there and no other, and every call to one reaches the
//! package's own part with its own state (the erased forms are made from
//! each trait's one definition): the gizmo it shows and its drags, its
//! time, its numeric bar, its tools' cancels, all on the fake host.
use kinogaki_studio_api::Vec3;
use kinogaki_studio_api::{ContextId, Package, PackageId};
use studio_ui_api::gizmo::{Handle, TransformPhase, TransformStep, TransformTool};
use studio_ui_api::host::{Cx, ViewCx, Waker};
use studio_ui_api::package::{
    BarField, BarValue, ContextPart, Ended, GizmoUse, LiveTools, NumericBar, NumericFields, Takes,
    TimeDomain, TimeUnits, Timeline, Transforms, UiPackage, WithUi,
};
use studio_ui_api::{Pos, Rect, Vec2};
use studio_ui_api_testing::{FakeHost, Hosting};

fn context() -> ContextId {
    ContextId::declared("acme.clock.clock")
}

/// What the clock's parts were told.
#[derive(Default)]
struct Clock {
    gizmo: GizmoUse,
    phases: Vec<TransformPhase>,
    at: f64,
    typed: Option<(usize, f64)>,
    ended: Vec<Ended>,
}

struct Parts;

impl LiveTools<Clock> for Parts {
    fn accept(&self, _s: &mut Clock, _cx: &mut dyn Cx) {}
    /// Its hand: the pointer's, while it is over the view.
    fn tools(&self, _s: &mut Clock, cx: &mut dyn ViewCx) -> Takes {
        let held = cx
            .input()
            .pointer
            .button_down(studio_ui_api::PointerButton::Primary);
        match held {
            true => Takes::ALL,
            false => Takes::default(),
        }
    }
    fn cancel(&self, s: &mut Clock, _cx: &mut dyn Cx, why: Ended) {
        s.ended.push(why);
    }
}

impl Transforms<Clock> for Parts {
    fn gizmo(&self, s: &Clock, _cx: &dyn Cx) -> GizmoUse {
        s.gizmo
    }
    fn transform(&self, s: &mut Clock, _cx: &mut dyn Cx, phase: TransformPhase, _: &TransformStep) {
        s.phases.push(phase);
    }
}

impl Timeline<Clock> for Parts {
    fn time(&self, s: &Clock, _cx: &dyn Cx) -> Option<TimeDomain> {
        Some(TimeDomain {
            range: (0.0, 60.0),
            units: TimeUnits::Clock,
            marks: Vec::new().into(),
            current: s.at,
            speeds: Vec::new(),
        })
    }
    fn set_time(&self, s: &mut Clock, _cx: &mut dyn Cx, t: f64) {
        s.at = t;
    }
}

impl NumericBar<Clock> for Parts {
    fn fields(&self, s: &Clock, _cx: &dyn Cx) -> Option<NumericFields> {
        Some(NumericFields {
            fields: vec![BarField {
                name: Some("At".into()),
                value: BarValue::Number(s.at),
                unavailable: None,
            }],
            unit: kinogaki_studio_api::params::Unit::Plain,
        })
    }
    fn apply(&self, s: &mut Clock, _cx: &mut dyn Cx, field: usize, _current: f64, value: f64) {
        s.typed = Some((field, value));
    }
}

struct Ui;

impl UiPackage for Ui {
    type State = Clock;
    fn state(&self, _wake: Waker) -> Clock {
        Clock::default()
    }
    fn contexts(&self) -> Vec<ContextPart<Clock>> {
        vec![
            ContextPart::new(context())
                .with_tools(Parts)
                .with_transforms(Parts)
                .with_timeline(Parts)
                .with_numeric(Parts),
        ]
    }
}

fn hosted() -> (FakeHost, Hosting) {
    let package = Package::new(PackageId::declared("acme.clock"), "Clock").with_ui(Ui);
    let host = FakeHost::new(context(), package.slots.clone());
    let hosting = Hosting::new(package, &host);
    (host, hosting)
}

#[test]
fn a_context_has_the_parts_it_gives_and_no_other() {
    let (_, hosting) = hosted();
    let context = &hosting.registered().contexts[0];
    assert!(context.tools.is_some() && context.transforms.is_some());
    assert!(context.timeline.is_some() && context.numeric.is_some());
    assert!(context.panels.is_none() && context.display.is_none() && context.levels.is_none());
}

#[test]
fn every_call_reaches_the_packages_own_part_with_its_state() {
    let (mut host, mut hosting) = hosted();
    hosting.with(|p, s| {
        let context = &p.contexts[0];
        let transforms = context.transforms.as_ref().unwrap();
        assert_eq!(transforms.gizmo(s, &host), GizmoUse::None);
        let step = TransformStep::none(
            TransformTool::Move,
            Handle::Axis(0),
            Vec3::ZERO,
            [
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(0.0, 0.0, 1.0),
            ],
        );
        transforms.transform(s, &mut host, TransformPhase::Began, &step);
        let timeline = context.timeline.as_ref().unwrap();
        timeline.set_time(s, &mut host, 12.5);
        assert_eq!(timeline.time(s, &host).map(|t| t.current), Some(12.5));
        let numeric = context.numeric.as_ref().unwrap();
        assert!(numeric.fields(s, &host).is_some());
        numeric.apply(s, &mut host, 0, 12.5, 3.0);
        context
            .tools
            .as_ref()
            .unwrap()
            .cancel(s, &mut host, Ended::Undo);
    });
    let clock = hosting.state::<Clock>();
    assert_eq!(clock.phases, [TransformPhase::Began]);
    assert_eq!((clock.at, clock.typed), (12.5, Some((0, 3.0))));
    assert_eq!(clock.ended, [Ended::Undo]);
}

#[test]
fn a_tool_reads_the_views_input_from_its_view_call() {
    let (mut host, mut hosting) = hosted();
    let rect = Rect::from_min_size(Pos::ZERO, Vec2::new(200.0, 100.0));
    host.view = Some(studio_ui_api_testing::view(rect));
    let mut take = |host: &mut FakeHost| {
        hosting.with(|p, s| p.contexts[0].tools.as_ref().unwrap().tools(s, host))
    };
    assert_eq!(take(&mut host), Takes::default());
    host.input.pointer.down[0] = true;
    assert_eq!(take(&mut host), Takes::ALL, "a held button: the tool's");
}
