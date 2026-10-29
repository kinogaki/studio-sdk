//! The cook contract through toy nodes over a toy "numbers" data type: a
//! Scale node (parameters, ports, parallel work, scratch, incremental
//! updates) and a held edit that adds an authored delta.
use kinogaki_studio_api::parallel::Serial;
use kinogaki_studio_api::*;
use std::any::Any;
use std::sync::{Arc, LazyLock};

static NUMBERS: LazyLock<Typed<Vec<f64>>> =
    LazyLock::new(|| Typed::new(DataTypeId::declared("toy.numbers.list")));
static OFFSETS: LazyLock<Typed<Vec<f64>>> =
    LazyLock::new(|| Typed::new(DataTypeId::declared("toy.numbers.offsets")));

thread_local! {
    /// Counts how often Scale really cooked (not from its plan), per test
    /// thread: tests run side by side, and cooks run on the calling thread.
    static FULL_COOKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Scale: every number of every wire on port 0 times `factor`, written
/// in parallel. Its scratch counts cooks of the node.
struct Scale;
impl Scale {
    fn compute(cx: &mut CookCx<'_>, inputs: &[Data]) -> Result<Outputs, CookError> {
        cx.check()?;
        let factor = cx.number("factor")?;
        let mut values: Vec<f64> = inputs[cx.port(0)]
            .iter()
            .filter_map(|d| NUMBERS.get(d))
            .flatten()
            .copied()
            .collect();
        cx.parallel_for(&mut values, 4, |_, block| {
            block.iter_mut().for_each(|v| *v *= factor);
            Ok::<_, CookError>(())
        })?;
        *cx.scratch::<usize>() += 1;
        let mut outputs = Outputs::one(NUMBERS.wrap(values));
        outputs.selection = cx.incoming().cloned();
        Ok(outputs)
    }
}
impl NodeType for Scale {
    fn id(&self) -> NodeTypeId {
        NodeTypeId::declared("toy.numbers.scale")
    }
    fn description(&self) -> Description {
        Description::new("Scale")
            .input(Port::new("numbers", "Numbers", NUMBERS.id()).multi())
            .output(Port::new("numbers", "Numbers", NUMBERS.id()))
            .parameters(Schema::default().parameter(params::Parameter::new(
                "factor",
                "Factor",
                params::ParameterKind::Number(params::Number::plain()),
                1.0,
            )))
    }
    fn cook(&self, cx: &mut CookCx<'_>, inputs: &[Data]) -> Result<Outputs, CookError> {
        FULL_COOKS.set(FULL_COOKS.get() + 1);
        Self::compute(cx, inputs)
    }
    fn incremental(&self) -> Option<&dyn Incremental> {
        Some(self)
    }
}
impl Incremental for Scale {
    fn prepare(&self, cx: &mut CookCx<'_>, inputs: &[Data]) -> Result<Prepared, CookError> {
        Ok(Prepared {
            outputs: self.cook(cx, inputs)?,
            plan: Some(Arc::new(ScalePlan {
                input: inputs[0].clone(),
            })),
        })
    }
}

/// Scale's plan: valid while its input is the same value.
struct ScalePlan {
    input: Data,
}
impl CookPlan for ScalePlan {
    fn update(&self, cx: &mut CookCx<'_>, inputs: &[Data]) -> Result<Option<Outputs>, CookError> {
        if !inputs[0].ptr_eq(&self.input) {
            return Ok(None);
        }
        Scale::compute(cx, inputs).map(Some)
    }
    fn memory(&self) -> usize {
        0
    }
}

/// Offsets: a held edit adding one authored offset per number, and the
/// first number of input 1 to every one when the delta is longer than the
/// base.
struct AddOffsets;
impl HeldEdit for AddOffsets {
    fn base(&self) -> DataTypeId {
        NUMBERS.id()
    }
    fn delta(&self) -> DataTypeId {
        OFFSETS.id()
    }
    fn reads_input(&self, delta: &Data, port: usize) -> bool {
        port == 1 && OFFSETS.get(delta).is_some_and(|d| d.len() > 2)
    }
    fn apply(
        &self,
        cx: &mut CookCx<'_>,
        base: &Data,
        delta: &Data,
        inputs: &[Option<Data>],
    ) -> Result<Outputs, CookError> {
        cx.check()?;
        let (base, delta) = NUMBERS
            .get(base)
            .zip(OFFSETS.get(delta))
            .ok_or_else(|| CookError::message("not numbers and offsets"))?;
        let lift = inputs
            .first()
            .and_then(Option::as_ref)
            .and_then(|d| NUMBERS.get(d))
            .and_then(|n| n.first())
            .unwrap_or(&0.0);
        Ok(Outputs::one(
            NUMBERS.wrap(
                base.iter()
                    .enumerate()
                    .map(|(i, v)| v + delta.get(i).unwrap_or(&0.0) + lift)
                    .collect(),
            ),
        ))
    }
}

fn numbers(values: &[f64]) -> Data {
    NUMBERS.wrap(values.to_vec())
}

fn factor(value: f64) -> Parameters {
    Parameters::from([("factor".to_owned(), Value::Number(value))])
}

#[test]
fn a_cook_reads_parameters_ports_and_scratch() {
    let parameters = factor(2.0);
    let cancel = CancelToken::default();
    let mut scratch: Option<Box<dyn Any + Send>> = None;
    let incoming = Components::new(NUMBERS.id(), Level::Point, 1, vec![0u32]);
    // Port 0 has two wires, port 1 one.
    let inputs = [numbers(&[1.0, 2.0]), numbers(&[3.0]), numbers(&[100.0])];
    for cooks in 1..=2 {
        let mut cx = CookCx::new(&parameters, &cancel, &Serial, &mut scratch)
            .with_ports(&[2, 1])
            .with_incoming(&incoming);
        assert_eq!((cx.port(0), cx.port(1), cx.port(2)), (0..2, 2..3, 3..3));
        let out = Scale.cook(&mut cx, &inputs).unwrap();
        assert_eq!(NUMBERS.get(&out.data[0]), Some(&vec![2.0, 4.0, 6.0]));
        assert!(out.selection.unwrap().ptr_eq(&incoming));
        assert_eq!(*cx.scratch::<usize>(), cooks);
    }
    // Scratch of another type replaces the slot.
    let mut cx = CookCx::new(&parameters, &cancel, &Serial, &mut scratch);
    assert_eq!(cx.scratch::<String>(), "");
    cx.clear_scratch();
    assert!(scratch.is_none());
}

#[test]
fn withheld_and_wrong_parameters_fail_by_name() {
    let parameters = Parameters::from([("factor".to_owned(), Value::Text("x".into()))]);
    let cancel = CancelToken::default();
    let mut scratch = None;
    let mut cx = CookCx::new(&parameters, &cancel, &Serial, &mut scratch);
    // Without ports each input is its own port; no names, no selection.
    assert_eq!(cx.port(1), 1..2);
    assert!(cx.input_name(0).is_none() && cx.selection().is_none());
    assert_eq!(cx.time(), Time::default());
    let error = Scale.cook(&mut cx, &[numbers(&[1.0])]).unwrap_err();
    assert!(error.to_string().contains("\"factor\" is not a number"));
    assert!(cx.toggle("missing").is_err());
    assert_eq!(cx.text("factor").unwrap(), "x");
}

#[test]
fn a_cancelled_cook_reports_cancelled() {
    let parameters = factor(1.0);
    let cancel = CancelToken::default();
    cancel.cancel();
    let mut scratch = None;
    let mut cx = CookCx::new(&parameters, &cancel, &Serial, &mut scratch);
    assert_eq!(
        Scale.cook(&mut cx, &[numbers(&[1.0])]).unwrap_err(),
        CookError::Cancelled
    );
    assert_eq!(CookError::from(Error::Cancelled), CookError::Cancelled);
}

#[test]
fn plans_update_until_their_input_changes() {
    let cancel = CancelToken::default();
    let mut scratch = None;
    let input = numbers(&[1.0, 2.0]);
    let parameters = factor(2.0);
    let mut cx = CookCx::new(&parameters, &cancel, &Serial, &mut scratch);
    let prepared = Scale
        .incremental()
        .unwrap()
        .prepare(&mut cx, std::slice::from_ref(&input))
        .unwrap();
    let plan = prepared.plan.unwrap();
    let before = FULL_COOKS.get();
    // A drag of the factor updates from the plan.
    let parameters = factor(3.0);
    let mut cx = CookCx::new(&parameters, &cancel, &Serial, &mut scratch);
    let updated = plan
        .update(&mut cx, std::slice::from_ref(&input))
        .unwrap()
        .unwrap();
    assert_eq!(NUMBERS.get(&updated.data[0]), Some(&vec![3.0, 6.0]));
    assert_eq!(FULL_COOKS.get(), before);
    // A new input ends the plan; it can't reprepare.
    let other = numbers(&[1.0, 2.0]);
    assert!(
        plan.update(&mut cx, std::slice::from_ref(&other))
            .unwrap()
            .is_none()
    );
    assert!(plan.reprepare(&mut cx, &[other]).unwrap().is_none());
}

/// Bare data types for the registry: no facets, a codec that refuses.
struct Bare(DataTypeId);
struct Refuse;
impl DataCodec for Refuse {
    fn write(&self, _: &Data, _: Option<&Data>, _: &mut dyn ElementWriter) -> Result<(), Error> {
        Err(Error::invalid("not saved"))
    }
    fn read(&self, _: &dyn ElementReader, _: Option<&Data>) -> Result<Data, Error> {
        Err(Error::invalid("not saved"))
    }
}
impl DataType for Bare {
    fn id(&self) -> DataTypeId {
        self.0
    }
    fn label(&self) -> &str {
        self.0.name()
    }
    fn wire_color(&self) -> Color {
        Color::rgb(0, 0, 0)
    }
    fn stamp(&self, _: &Data) -> Stamp {
        [0; 32]
    }
    fn memory(&self, _: &Data) -> usize {
        0
    }
    fn bounds(&self, _: &Data) -> Option<Bounds> {
        None
    }
    fn info(&self, _: &Data) -> Vec<InfoRow> {
        Vec::new()
    }
    fn codec(&self) -> &dyn DataCodec {
        &Refuse
    }
}

fn numbers_package() -> Package {
    Package::new(PackageId::declared("toy.numbers"), "Numbers")
        .with_data_type(Bare(NUMBERS.id()))
        .with_data_type(Bare(OFFSETS.id()))
        .with_held_edit(AddOffsets)
        .with_node_type(Scale)
}

#[test]
fn the_registry_holds_cooks_and_held_edits() {
    let mut registry = Registry::new();
    registry.register(numbers_package()).unwrap();
    let scale = registry
        .node_type(NodeTypeId::declared("toy.numbers.scale"))
        .unwrap();
    assert_eq!(scale.reads(&factor(2.0)), Reads::default());
    // Its description: what a new node holds.
    let description = scale.description();
    assert_eq!(description.label, "Scale");
    assert!(description.inputs[0].multi && !description.outputs[0].multi);
    assert_eq!(description.parameters.values(), factor(1.0));
    assert!(
        registry
            .node_type(NodeTypeId::declared("toy.numbers.other"))
            .is_none()
    );
    let held = registry.held_edit(NUMBERS.id()).unwrap();
    let (cancel, mut scratch, none) = (CancelToken::default(), None, Parameters::new());
    let mut cx = CookCx::new(&none, &cancel, &Serial, &mut scratch);
    let short = OFFSETS.wrap(vec![0.5]);
    assert!(!held.reads_input(&short, 1));
    let edited = held
        .apply(&mut cx, &numbers(&[1.0, 2.0]), &short, &[None])
        .unwrap();
    assert_eq!(NUMBERS.get(&edited.data[0]), Some(&vec![1.5, 2.0]));
    // A longer delta reads input 1 too.
    let long = OFFSETS.wrap(vec![0.0; 3]);
    assert!(held.reads_input(&long, 1));
    let lifted = held
        .apply(&mut cx, &numbers(&[1.0]), &long, &[Some(numbers(&[10.0]))])
        .unwrap();
    assert_eq!(NUMBERS.get(&lifted.data[0]), Some(&vec![11.0]));
    assert!(registry.held_edit(OFFSETS.id()).is_none());
    // A second package can't declare held edits of the same type.
    let again = Package::new(PackageId::declared("toy.again"), "Again").with_held_edit(AddOffsets);
    assert_eq!(
        registry.register(again).unwrap_err(),
        RegistryError::DuplicateHeldEdit {
            package: PackageId::declared("toy.again"),
            base: NUMBERS.id(),
            first: PackageId::declared("toy.numbers"),
        }
    );
    // Nor the same node type.
    let twice = Package::new(PackageId::declared("toy.twice"), "Twice").with_node_type(Scale);
    assert!(matches!(
        registry.register(twice),
        Err(RegistryError::DuplicateId {
            what: "node type",
            ..
        })
    ));
}

#[test]
fn held_edits_need_their_types_registered() {
    let mut registry = Registry::new();
    let alone = Package::new(PackageId::declared("toy.alone"), "Alone").with_held_edit(AddOffsets);
    assert!(matches!(
        registry.register(alone),
        Err(RegistryError::UnknownDataType {
            by: "held edit",
            ..
        })
    ));
}
