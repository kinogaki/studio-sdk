//! The API through a toy kernel: a point cloud data type (codec, display,
//! no other facet), a "count" type a bridge converts clouds into, a Cloud
//! object kind, a format and a selection slot, registered as packages.
use kinogaki_studio_api::element::MemoryElement;
use kinogaki_studio_api::*;
use std::sync::{Arc, LazyLock};

/// The toy kernel's value.
#[derive(Debug, PartialEq)]
struct Cloud(Vec<[f64; 3]>);

static CLOUD: LazyLock<Typed<Cloud>> =
    LazyLock::new(|| Typed::new(DataTypeId::declared("toy.points.cloud")));
static COUNT: LazyLock<Typed<usize>> =
    LazyLock::new(|| Typed::new(DataTypeId::declared("toy.points.count")));

struct CloudCodec;
impl DataCodec for CloudCodec {
    fn write(
        &self,
        data: &Data,
        _base: Option<&Data>,
        out: &mut dyn ElementWriter,
    ) -> Result<(), Error> {
        let cloud = CLOUD
            .get(data)
            .ok_or_else(|| Error::invalid("not a cloud"))?;
        let flat: Vec<f64> = cloud.0.iter().flatten().copied().collect();
        out.set_rows("points", 3, Field::F64s(flat))
    }
    fn read(&self, element: &dyn ElementReader, _base: Option<&Data>) -> Result<Data, Error> {
        let flat = element.f64s("points")?;
        Ok(CLOUD.wrap(Cloud(flat.as_chunks::<3>().0.to_vec())))
    }
}

struct CloudDisplay;
impl Displayable for CloudDisplay {
    fn levels(&self) -> &[Level] {
        &[Level::Point]
    }
    fn prepare(
        &self,
        data: &Data,
        _quality: DisplayQuality,
        cancel: &CancelToken,
    ) -> Result<Drawn, Error> {
        cancel.check()?;
        let cloud = CLOUD
            .get(data)
            .ok_or_else(|| Error::invalid("not a cloud"))?;
        // Points as markers, no triangles.
        let points: Arc<[[f32; 3]]> = cloud.0.iter().map(|p| p.map(|c| c as f32)).collect();
        let markers: Arc<[[u32; 2]]> = (0..points.len() as u32).map(|i| [i, i]).collect();
        let surface =
            Surface::new(points, Arc::from([]), Arc::from([]), 0, 0).with_markers(markers);
        Ok(Drawn::new(surface).with_detail(cloud.0.len()))
    }
    fn highlight(&self, _data: &Data, _drawn: &Drawn, components: &Components) -> Elements {
        let ids = components.get::<Vec<u32>>().cloned().unwrap_or_default();
        Elements::new(Level::Point, ids)
    }
    fn convert(&self, _: &Data, components: &Components, to: Level) -> Option<Components> {
        (to == Level::Point).then(|| components.clone())
    }
}

struct CloudType;
impl DataType for CloudType {
    fn id(&self) -> DataTypeId {
        CLOUD.id()
    }
    fn label(&self) -> &str {
        "Cloud"
    }
    fn wire_color(&self) -> Color {
        Color::rgb(90, 160, 220)
    }
    fn stamp(&self, data: &Data) -> Stamp {
        let mut stamp = [0; 32];
        stamp[..8].copy_from_slice(
            &CLOUD
                .get(data)
                .map_or(0, |c| c.0.len() as u64)
                .to_le_bytes(),
        );
        stamp
    }
    fn memory(&self, data: &Data) -> usize {
        CLOUD.get(data).map_or(0, |c| c.0.len() * 24)
    }
    fn bounds(&self, data: &Data) -> Option<Bounds> {
        let points = CLOUD.get(data)?.0.iter().map(|&p| Vec3::from_array(p));
        points.map(Bounds::point).reduce(Bounds::union)
    }
    fn info(&self, data: &Data) -> Vec<InfoRow> {
        let count = CLOUD.get(data).map_or(0, |c| c.0.len());
        vec![InfoRow::new("Points", count.to_string())]
    }
    fn codec(&self) -> &dyn DataCodec {
        &CloudCodec
    }
    fn displayable(&self) -> Option<&dyn Displayable> {
        Some(&CloudDisplay)
    }
}

/// A second toy type: a bare count, with no codec content and no facets.
struct CountType;
impl DataType for CountType {
    fn id(&self) -> DataTypeId {
        COUNT.id()
    }
    fn label(&self) -> &str {
        "Count"
    }
    fn wire_color(&self) -> Color {
        Color::rgb(200, 200, 200)
    }
    fn stamp(&self, _: &Data) -> Stamp {
        [0; 32]
    }
    fn memory(&self, _: &Data) -> usize {
        8
    }
    fn bounds(&self, _: &Data) -> Option<Bounds> {
        None
    }
    fn info(&self, _: &Data) -> Vec<InfoRow> {
        Vec::new()
    }
    fn codec(&self) -> &dyn DataCodec {
        &CloudCodec
    }
}

/// The bridge: a cloud as its point count.
struct Counting;
impl Conversion for Counting {
    fn id(&self) -> ConversionId {
        ConversionId::declared("toy.bridge.count")
    }
    fn label(&self) -> &str {
        "Count"
    }
    fn from(&self) -> DataTypeId {
        CLOUD.id()
    }
    fn to(&self) -> DataTypeId {
        COUNT.id()
    }
    fn convert(&self, data: &Data, cancel: &CancelToken) -> Result<Data, Error> {
        cancel.check()?;
        let cloud = CLOUD
            .get(data)
            .ok_or_else(|| Error::invalid("not a cloud"))?;
        Ok(COUNT.wrap(cloud.0.len()))
    }
}

/// A Cloud object's body: the cloud it holds.
#[derive(Debug)]
struct CloudBody(Data);
impl ObjectBody for CloudBody {
    fn kind(&self) -> ObjectKindId {
        ObjectKindId::declared("toy.points.object")
    }
}

struct CloudBodyCodec;
impl ObjectCodec for CloudBodyCodec {
    fn write(&self, body: &dyn ObjectBody, out: &mut dyn ElementWriter) -> Result<(), Error> {
        let body = body
            .downcast_ref::<CloudBody>()
            .ok_or_else(|| Error::invalid("not a Cloud object"))?;
        CloudCodec.write(&body.0, None, &mut *out.child("cloud", "cloud")?)
    }
    fn read(&self, element: &dyn ElementReader) -> Result<Arc<dyn ObjectBody>, Error> {
        let cloud = CloudCodec.read(&*element.required_child("cloud")?, None)?;
        Ok(Arc::new(CloudBody(cloud)))
    }
}

struct CloudObject;
impl ObjectKindSpec for CloudObject {
    fn id(&self) -> ObjectKindId {
        ObjectKindId::declared("toy.points.object")
    }
    fn label(&self) -> &str {
        "Cloud"
    }
    fn icon(&self) -> &str {
        "points"
    }
    fn placed(&self) -> bool {
        true
    }
    fn context(&self) -> Option<ContextId> {
        None
    }
    fn result(&self) -> Option<DataTypeId> {
        Some(CLOUD.id())
    }
    fn new_body(&self) -> Arc<dyn ObjectBody> {
        Arc::new(CloudBody(CLOUD.wrap(Cloud(Vec::new()))))
    }
    fn codec(&self) -> &dyn ObjectCodec {
        &CloudBodyCodec
    }
    fn adopts(&self, data: &Data) -> Option<Arc<dyn ObjectBody>> {
        CLOUD
            .is(data)
            .then(|| Arc::new(CloudBody(data.clone())) as Arc<dyn ObjectBody>)
    }
}

/// A format that "reads" one fixed cloud.
struct XyzFormat;
impl FileFormat for XyzFormat {
    fn id(&self) -> FormatId {
        FormatId::declared("toy.format.xyz")
    }
    fn label(&self) -> &str {
        "XYZ points"
    }
    fn extensions(&self) -> &[&str] {
        &["xyz"]
    }
    fn produces(&self) -> &[DataTypeId] {
        static PRODUCES: LazyLock<[DataTypeId; 1]> = LazyLock::new(|| [CLOUD.id()]);
        &*PRODUCES
    }
    fn read(
        &self,
        _path: &std::path::Path,
        _options: &FormatOptions,
        reading: &Reading<'_>,
    ) -> Result<Imported, Error> {
        reading.check()?;
        Ok(Imported {
            items: vec![FileItem::top(
                "cloud",
                CLOUD.wrap(Cloud(vec![[1.0, 2.0, 3.0]])),
            )],
            warnings: Vec::new(),
        })
    }
}

struct Selection;
impl SessionSlot for Selection {
    fn id(&self) -> SlotId {
        SlotId::declared("toy.points.selection")
    }
    fn initial(&self) -> SlotValue {
        SlotValue::new(Vec::<u32>::new())
    }
}

fn kernel() -> Package {
    Package::new(PackageId::declared("toy.points"), "Points")
        .with_data_type(CloudType)
        .with_data_type(CountType)
        .with_object_kind(CloudObject)
        .with_format(XyzFormat)
        .with_slot(Selection)
}

fn bridge() -> Package {
    Package::new(PackageId::declared("toy.bridge"), "Counting").with_conversion(Counting)
}

fn cloud(points: &[[f64; 3]]) -> Data {
    CLOUD.wrap(Cloud(points.to_vec()))
}

#[test]
fn the_environment_reaches_values_only_through_their_type() {
    let mut registry = Registry::new();
    registry.register(kernel()).unwrap();
    let data = cloud(&[[0.0, 0.0, 0.0], [2.0, 1.0, -1.0]]);
    let ty = registry.data_type(data.ty()).unwrap();
    assert_eq!(ty.label(), "Cloud");
    let bounds = ty.bounds(&data).unwrap();
    assert_eq!(
        (bounds.min, bounds.max),
        (Vec3::new(0.0, 0.0, -1.0), Vec3::new(2.0, 1.0, 0.0))
    );
    assert_eq!(ty.info(&data), vec![InfoRow::new("Points", "2")]);
    assert_eq!(ty.memory(&data), 48);
    // A clone is the same value; an equal new one isn't, but stamps agree.
    let other = cloud(&[[0.0, 0.0, 0.0], [2.0, 1.0, -1.0]]);
    assert!(data.ptr_eq(&data.clone()) && !data.ptr_eq(&other));
    assert_eq!(ty.stamp(&data), ty.stamp(&other));
}

#[test]
fn facets_a_type_does_not_offer_are_absent() {
    let mut registry = Registry::new();
    registry.register(kernel()).unwrap();
    let cloud_type = registry.data_type(CLOUD.id()).unwrap();
    assert!(cloud_type.displayable().is_some());
    assert!(cloud_type.pickable().is_none());
    assert!(cloud_type.tabular().is_none());
    assert!(cloud_type.renderable().is_none());
    let count_type = registry.data_type(COUNT.id()).unwrap();
    assert!(count_type.displayable().is_none());
}

#[test]
fn display_prepares_highlights_and_cancels() {
    let display = CloudDisplay;
    let data = cloud(&[[0.0; 3], [1.0; 3], [2.0; 3]]);
    let cancel = CancelToken::default();
    let drawn = display
        .prepare(&data, DisplayQuality::default(), &cancel)
        .unwrap();
    assert_eq!(drawn.detail::<usize>(), Some(&3));
    assert!(drawn.detail::<String>().is_none());
    assert_eq!(drawn.surface.markers().len(), 3);
    assert_eq!(drawn.surface.points()[2], [2.0; 3]);
    let picked = Components::new(CLOUD.id(), Level::Point, 2, vec![2u32, 0]);
    assert_eq!(display.highlight(&data, &drawn, &picked).ids(), &[0, 2]);
    assert!(display.convert(&data, &picked, Level::Face).is_none());
    cancel.cancel();
    assert_eq!(
        display.prepare(&data, DisplayQuality::Fine, &cancel).err(),
        Some(Error::Cancelled)
    );
}

#[test]
fn data_saves_into_a_project_and_reads_back() {
    let data = cloud(&[[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]);
    let mut element = MemoryElement::new(CLOUD.id().as_str());
    element
        .write(|out| CloudType.codec().write(&data, None, out))
        .unwrap();
    let back = element.read(|r| CloudType.codec().read(r, None)).unwrap();
    assert_eq!(CLOUD.get(&back), CLOUD.get(&data));
    let empty = MemoryElement::new(CLOUD.id().as_str());
    let missing = empty.read(|r| CloudType.codec().read(r, None)).unwrap_err();
    assert!(matches!(missing, Error::Invalid(m) if m.contains("points")));
}

#[test]
fn objects_adopt_imported_values_of_their_result_type() {
    let mut registry = Registry::new();
    registry.register(kernel()).unwrap();
    let format = registry
        .format(FormatId::declared("toy.format.xyz"))
        .unwrap();
    let imported = format
        .read(
            std::path::Path::new("in.xyz"),
            &format.options(),
            &Reading::new(Purpose::Objects, Units::default()),
        )
        .unwrap();
    let value = imported.items[0].data.as_ref().unwrap();
    let kind = registry
        .object_kinds()
        .find(|k| k.adopts(value).is_some())
        .unwrap();
    assert_eq!(kind.id(), ObjectKindId::declared("toy.points.object"));
    let body = kind.adopts(value).unwrap();
    assert!(body.downcast_ref::<CloudBody>().unwrap().0.ptr_eq(value));
    assert!(kind.adopts(&COUNT.wrap(1)).is_none());
    // The body round-trips through its kind's codec.
    let mut element = MemoryElement::new(kind.id().as_str());
    element
        .write(|out| kind.codec().write(&*body, out))
        .unwrap();
    let back = element.read(|r| kind.codec().read(r)).unwrap();
    let back = back.downcast_ref::<CloudBody>().unwrap();
    assert_eq!(CLOUD.get(&back.0), CLOUD.get(value));
    assert_eq!(
        registry
            .formats_producing(CLOUD.id())
            .map(|f| f.id())
            .collect::<Vec<_>>(),
        [format.id()]
    );
    assert_eq!(registry.formats_producing(COUNT.id()).count(), 0);
}

#[test]
fn conversions_come_from_bridges() {
    let mut registry = Registry::new();
    registry.register(kernel()).unwrap();
    assert!(
        registry
            .conversion_between(CLOUD.id(), COUNT.id())
            .is_none()
    );
    registry.register(bridge()).unwrap();
    let count = registry.conversion_between(CLOUD.id(), COUNT.id()).unwrap();
    let converted = count
        .convert(&cloud(&[[0.0; 3]; 4]), &CancelToken::default())
        .unwrap();
    assert_eq!(COUNT.get(&converted), Some(&4));
    assert!(
        registry
            .conversion_between(COUNT.id(), CLOUD.id())
            .is_none()
    );
}

#[test]
fn session_slots_start_initial_and_replace_whole() {
    let mut registry = Registry::new();
    registry.register(kernel()).unwrap();
    let id = SlotId::declared("toy.points.selection");
    let mut session = registry.new_session();
    assert_eq!(session.get_as::<Vec<u32>>(id), Some(&Vec::new()));
    let before = session.clone();
    let previous = session.set(id, SlotValue::new(vec![1u32, 2])).unwrap();
    assert!(previous.ptr_eq(before.get(id).unwrap()));
    assert_eq!(session.get_as::<Vec<u32>>(id), Some(&vec![1, 2]));
    // The snapshot is unchanged: an undo restores it.
    assert_eq!(before.get_as::<Vec<u32>>(id), Some(&Vec::new()));
    let unknown = SlotId::declared("toy.points.unknown");
    assert!(session.set(unknown, SlotValue::new(0u8)).is_none());
    assert!(session.get(unknown).is_none());
}

#[test]
fn a_duplicate_id_refuses_the_whole_package() {
    let mut registry = Registry::new();
    registry.register(kernel()).unwrap();
    // The same package again.
    assert_eq!(
        registry.register(kernel()).unwrap_err(),
        RegistryError::DuplicatePackage(PackageId::declared("toy.points"))
    );
    // Another package declaring the cloud type again, beside a new slot.
    let copy = Package::new(PackageId::declared("toy.copy"), "Copy")
        .with_slot(Selection2)
        .with_data_type(CloudType);
    assert_eq!(
        registry.register(copy).unwrap_err(),
        RegistryError::DuplicateId {
            package: PackageId::declared("toy.copy"),
            what: "data type",
            id: "toy.points.cloud",
            first: PackageId::declared("toy.points"),
        }
    );
    // Nothing of the refused package was kept.
    assert!(
        registry
            .slot(SlotId::declared("toy.copy.selection"))
            .is_none()
    );
    assert_eq!(registry.packages().count(), 1);
    // A package declaring one id twice.
    let twice = Package::new(PackageId::declared("toy.twice"), "Twice")
        .with_slot(Selection2)
        .with_slot(Selection2);
    assert!(matches!(
        registry.register(twice),
        Err(RegistryError::DuplicateId {
            what: "session slot",
            ..
        })
    ));
}

/// A slot a second package declares.
struct Selection2;
impl SessionSlot for Selection2 {
    fn id(&self) -> SlotId {
        SlotId::declared("toy.copy.selection")
    }
    fn initial(&self) -> SlotValue {
        SlotValue::new(())
    }
}

#[test]
fn packages_declare_only_their_vendors_ids() {
    let mut registry = Registry::new();
    let foreign =
        Package::new(PackageId::declared("acme.points"), "Points").with_data_type(CloudType);
    assert_eq!(
        registry.register(foreign).unwrap_err(),
        RegistryError::ForeignId {
            package: PackageId::declared("acme.points"),
            id: "toy.points.cloud",
        }
    );
}

#[test]
fn named_data_types_must_be_registered_first() {
    let mut registry = Registry::new();
    // The bridge before the kernel it converts between.
    let error = registry.register(bridge()).unwrap_err();
    assert_eq!(
        error,
        RegistryError::UnknownDataType {
            package: PackageId::declared("toy.bridge"),
            by: "toy.bridge.count",
            data: CLOUD.id(),
        }
    );
    assert!(error.to_string().contains("isn't registered"));
    registry.register(kernel()).unwrap();
    registry.register(bridge()).unwrap();
    // A second route between the same types is ambiguous.
    let again = Package::new(PackageId::declared("toy.again"), "Again").with_conversion(Again);
    assert!(matches!(
        registry.register(again),
        Err(RegistryError::DuplicateConversion { .. })
    ));
}

/// Another conversion from clouds to counts.
struct Again;
impl Conversion for Again {
    fn id(&self) -> ConversionId {
        ConversionId::declared("toy.again.count")
    }
    fn label(&self) -> &str {
        "Count again"
    }
    fn from(&self) -> DataTypeId {
        CLOUD.id()
    }
    fn to(&self) -> DataTypeId {
        COUNT.id()
    }
    fn convert(&self, data: &Data, cancel: &CancelToken) -> Result<Data, Error> {
        Counting.convert(data, cancel)
    }
}
