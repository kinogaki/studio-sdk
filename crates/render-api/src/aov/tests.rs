//! A final frame as an Image: each output class as its layer and sample, and
//! Cryptomatte's ranks and manifest.
use super::*;
use crate::geometry::Domain;

fn desc(id: &str, format: AovFormat) -> AovDesc {
    let aov =
        standard::get(id).unwrap_or_else(|| Aov::new(id, id, AovClass::Cryptomatte { levels: 3 }));
    AovDesc { aov, format }
}

/// What a layer is: its name, channels, sample and encoding.
fn shape(image: &Image, at: usize) -> (String, String, Sample, Encoding) {
    let format = image.layers()[at].format();
    let channels: Vec<&str> = format.channels.iter().map(|c| &**c).collect();
    (
        format.name.to_string(),
        channels.join(" "),
        format.sample,
        format.encoding.clone(),
    )
}

#[test]
fn each_output_class_is_its_layer_and_sample() {
    let pixels = 2;
    let plane = |desc: &AovDesc| match desc.aov.class {
        AovClass::Id => Values::Whole((0..pixels as u32).map(|v| v * 7 + 1).collect()),
        _ => Values::Floats(
            (0..pixels * desc.aov.channels())
                .map(|v| v as f32 * 0.25)
                .collect(),
        ),
    };
    let outputs: Vec<AovDesc> = [
        (standard::BEAUTY, AovFormat::F32),
        (standard::ALBEDO, AovFormat::F16),
        (standard::VOLUME, AovFormat::F32),
        (standard::NORMAL, AovFormat::F32),
        (standard::DEPTH, AovFormat::F32),
        (standard::OCCLUSION, AovFormat::F16),
        (standard::OBJECT_ID, AovFormat::U32),
    ]
    .into_iter()
    .map(|(id, format)| desc(id, format))
    .collect();
    let planes: Vec<_> = outputs.iter().map(|d| (d.clone(), plane(d))).collect();
    let image = image([2, 1], &planes, &[]).unwrap();
    let color = |premultiplied| Encoding::color(RENDER_SPACE, premultiplied);
    let expected = [
        ("", "R G B A", Sample::F32, color(true)),
        ("albedo", "R G B", Sample::F16, color(false)),
        ("volume", "R G B A", Sample::F32, color(true)),
        ("normal", "X Y Z", Sample::F32, Encoding::Raw),
        ("depth", "Z", Sample::F32, Encoding::Raw),
        ("occlusion", "Y", Sample::F16, Encoding::Raw),
        ("objectId", "id", Sample::U32, Encoding::Raw),
    ];
    for (at, (name, channels, sample, encoding)) in expected.into_iter().enumerate() {
        let made = (name.to_owned(), channels.to_owned(), sample, encoding);
        assert_eq!(shape(&image, at), made, "layer {at}");
    }
    assert_eq!(image.data().size(), [2, 1]);
    assert_eq!(image.read_pixel(0, 1, 0), [1.0, 1.25, 1.5, 1.75]);
    assert_eq!(image.read_pixel(3, 1, 0)[..3], [0.75, 1.0, 1.25]);
    // Ids are whole numbers, as they were given.
    let ids = image.plane(6).unwrap();
    assert_eq!(ids[4..8], 8u32.to_ne_bytes());
    assert_eq!(
        (layer(standard::BEAUTY), output("")),
        ("", standard::BEAUTY)
    );
    assert_eq!((layer("normal"), output("normal")), ("normal", "normal"));
}

#[test]
fn cryptomatte_ranks_fill_layers_of_two_and_name_what_they_hold() {
    // 2 × 1 pixels of three ranks: the left wholly /hero, the right 3/4 /floor.
    let ranks: Arc<[f32]> = vec![
        cryptomatte::id("/hero"),
        1.0,
        0.0,
        0.0,
        0.0,
        0.0,
        cryptomatte::id("/floor"),
        0.75,
        0.0,
        0.0,
        0.0,
        0.0,
    ]
    .into();
    let names: Vec<Arc<str>> = ["/floor", "/hero", "/Looks/Red", "/hero"]
        .map(Arc::from)
        .to_vec();
    let planes = [(desc("CryptoObject", AovFormat::F16), Values::Floats(ranks))];
    let image = image([2, 1], &planes, &names).unwrap();
    assert_eq!(image.layers().len(), 2, "three ranks, two layers");
    let raw = |name: &str| {
        (
            name.to_owned(),
            "R G B A".to_owned(),
            Sample::F32,
            Encoding::Raw,
        )
    };
    assert_eq!(shape(&image, 0), raw("CryptoObject00"));
    assert_eq!(shape(&image, 1), raw("CryptoObject01"));
    let right = image.read_pixel(0, 1, 0);
    assert_eq!(
        (right[0].to_bits(), right[1]),
        (cryptomatte::id("/floor").to_bits(), 0.75)
    );
    assert_eq!(image.read_pixel(1, 0, 0), [0.0; 4], "the third rank, empty");
    let text = |key: &str| {
        let name = cryptomatte::metadata("CryptoObject", key);
        let found = image.attributes().text(Domain::Detail, &name);
        found.and_then(|t| t.get(0)).map(str::to_owned)
    };
    assert_eq!(text("name").as_deref(), Some("CryptoObject"));
    assert_eq!(text("hash").as_deref(), Some("MurmurHash3_32"));
    assert_eq!(text("conversion").as_deref(), Some("uint32_to_float32"));
    // The names the layers hold, in order, once each: the specification's hashes.
    let floor = format!("{:08x}", cryptomatte::id("/floor").to_bits());
    let manifest = text("manifest").unwrap();
    assert!(manifest.starts_with("{\"/floor\":\"") && manifest.contains("\"/hero\":\""));
    assert!(!manifest.contains("Looks"), "{manifest}");
    assert_eq!(manifest.matches("hero").count(), 1);
    assert_eq!(&manifest[11..19], floor);
    assert!(cryptomatte::metadata("CryptoObject", "name").starts_with("cryptomatte/"));
}

#[test]
fn the_specifications_hashes_and_keys() {
    // Psyop's vectors: MurmurHash3_32 of "torus", and the fixtures' keys.
    assert_eq!(cryptomatte::id("torus").to_bits(), 4_053_562_365);
    assert_eq!(
        cryptomatte::metadata("crypto_asset", "name"),
        "cryptomatte/28322e9/name"
    );
    assert_eq!(
        cryptomatte::metadata("crypto_object", "x"),
        "cryptomatte/f834d0a/x"
    );
    // A hash whose exponent is 0 or 255 moves off it: no denormal, no NaN.
    assert!(
        cryptomatte::id("").is_normal(),
        "the empty name's hash is 0"
    );
}

#[test]
fn values_not_of_the_frame_are_refused() {
    let beauty = desc(standard::BEAUTY, AovFormat::F32);
    let short = Values::Floats(vec![0.0; 3].into());
    assert!(image([2, 1], &[(beauty.clone(), short)], &[]).is_err());
    let ids = desc(standard::OBJECT_ID, AovFormat::U32);
    let floats = Values::Floats(vec![0.0; 2].into());
    assert!(
        image([2, 1], &[(ids, floats)], &[]).is_err(),
        "ids as floats"
    );
    let beauty = Values::Floats(vec![0.0; 4].into());
    assert!(image([0, 1], &[], &[]).is_err(), "a frame of no pixels");
    assert!(
        image(
            [1, 1],
            &[(desc(standard::BEAUTY, AovFormat::F32), beauty)],
            &[]
        )
        .is_ok()
    );
}
