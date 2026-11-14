use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Event, Runner};
use std::path::Path;

struct Empty;
impl DataSource for Empty {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
}

fn plan(name: &str) -> exact_plan::Plan {
    contract::compile_path(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../corpus/derive-form")
            .join(name),
    )
    .unwrap()
}

#[test]
fn component_form_stays_within_ten_percent_of_its_root_twin_and_edits_identically() {
    let child = plan("components.contract");
    let root = plan("root.contract");
    assert!(child.derives.iter().any(|d| d.unsettled));
    assert!(
        child.code.len() * 100 <= root.code.len() * 110,
        "child {} / root {}",
        child.code.len(),
        root.code.len()
    );
    assert!(
        child.encode().len() * 100 <= root.encode().len() * 110,
        "encoded child {} / root {}",
        child.encode().len(),
        root.encode().len()
    );
    let boot =
        |p| Runner::boot(p, Empty, Kernel::with_monospace(), Default::default(), "/").unwrap();
    let (mut a, mut b) = (boot(child), boot(root));
    assert_eq!(
        a.kernel().export(None).unwrap(),
        b.kernel().export(None).unwrap()
    );
    let mut seed = 0x63d5_u64;
    for step in 0..100 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let (id, event) = match step % 5 {
            0 => (
                "field".to_string(),
                Event::Input(format!("label {step}").into()),
            ),
            1 => ("field".to_string(), Event::Input("x".into())),
            2 => ("toggle".to_string(), Event::Press),
            3 => ("rekey".to_string(), Event::Press),
            _ => (
                format!("edit{}", seed % 19),
                Event::Input(format!("draft {step}").into()),
            ),
        };
        let view = |r: &Runner<Empty>| {
            r.kernel()
                .node_by_key(r.kernel().find_by_test_id(&id)[0])
                .unwrap()
                .id
        };
        let before = a.kernel().export(None).unwrap();
        let ar = a.dispatch(view(&a), event.clone()).unwrap();
        let br = b.dispatch(view(&b), event).unwrap();
        assert_eq!(format!("{ar:?}"), format!("{br:?}"));
        assert_eq!(
            a.kernel().export(None).unwrap(),
            b.kernel().export(None).unwrap()
        );
        if step == 0 {
            assert_ne!(before, a.kernel().export(None).unwrap());
            assert!(exact_runner::agent::tree(&a).contains("label 0"));
        }
        if step == 1 {
            assert!(!exact_runner::agent::tree(&a).contains("label 0"));
        }
        assert!(a.journal().all(|line| !line.contains("trapped")));
        assert!(b.journal().all(|line| !line.contains("trapped")));
    }
}

#[test]
fn derives_in_stateless_arms_keep_their_owner_through_deeper_frames() {
    let plan = plan("stateless.contract");
    let mut runner = Runner::boot(
        plan,
        Empty,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let view = runner
        .kernel()
        .node_by_key(runner.kernel().find_by_test_id("change")[0])
        .unwrap()
        .id;
    runner.dispatch(view, Event::Press).unwrap();
    assert!(!runner.is_poisoned());
    assert!(runner.journal().all(|line| !line.contains("trapped")));
    let tree = exact_runner::agent::tree(&runner);
    assert!(tree.contains("second!") && tree.contains("c!"), "{tree}");
}

#[test]
fn sharing_dependencies_does_not_depend_on_declaration_order() {
    let source = |derives: &str| {
        format!("component App\n  state items = [\"one\", \"two\"]\n  view\n    Child(items=items)\ncomponent Child\n  props\n    items: string[]\n{derives}  view\n    text `${{a}} ${{b}} ${{expensive.length}}`\n")
    };
    let declarations = [
        "  derive expensive = items.filter(x => x.length > 2)\n",
        "  derive a = expensive.length\n",
        "  derive b = expensive.join(\",\")\n",
    ];
    let forward = contract::compile(&source(&declarations.concat())).unwrap();
    let reverse = contract::compile(&source(
        &[declarations[1], declarations[2], declarations[0]].concat(),
    ))
    .unwrap();
    assert_eq!(forward.code.len(), reverse.code.len());
    assert_eq!(forward.encode().len(), reverse.encode().len());
}
