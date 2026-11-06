//! Cryptomatte 1.2 (Psyop's specification) for every engine: how a name
//! becomes an ID, and the manifest an Image carries as metadata so a reader
//! names what each ID is.
//!
//! An output of `levels` ranks is `levels.div_ceil(2)` layers of a frame's
//! Image, `<id>00`, `<id>01`…, each `R G B A`: two ranks of (ID, coverage),
//! the ID a name's hash as float bits ([`id`]). Its manifest is four Detail
//! texts under the specification's keys ([`metadata`]): `name`, `hash`
//! (`MurmurHash3_32`), `conversion` (`uint32_to_float32`) and `manifest`
//! (JSON: each name and its hash as eight hex digits).
use crate::geometry::{Attribute, Attributes, Domain, Error, Text};
use std::collections::BTreeSet;
use std::sync::Arc;

/// MurmurHash3_x86_32 of `bytes` with seed 0, the specification's hash.
fn murmur3_32(bytes: &[u8]) -> u32 {
    const C1: u32 = 0xCC9E_2D51;
    const C2: u32 = 0x1B87_3593;
    let mix = |k: u32| k.wrapping_mul(C1).rotate_left(15).wrapping_mul(C2);
    let mut h = 0u32;
    let (blocks, tail) = bytes.as_chunks::<4>();
    for block in blocks {
        h = (h ^ mix(u32::from_le_bytes(*block)))
            .rotate_left(13)
            .wrapping_mul(5)
            .wrapping_add(0xE654_6B64);
    }
    if !tail.is_empty() {
        let k = (tail.iter().enumerate()).fold(0u32, |k, (i, &b)| k | u32::from(b) << (8 * i));
        h ^= mix(k);
    }
    h ^= bytes.len() as u32;
    h ^= h >> 16;
    h = h.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 13;
    h = h.wrapping_mul(0xC2B2_AE35);
    h ^ (h >> 16)
}

/// A hash's ID bits: its exponent moved off 0 and 255, so the float is
/// neither denormal, infinite nor NaN.
fn id_bits(hash: u32) -> u32 {
    match (hash >> 23) & 0xFF {
        0 | 255 => hash ^ 1 << 23,
        _ => hash,
    }
}

/// The ID a layer holds for `name`: its hash's bits as a float.
pub fn id(name: &str) -> f32 {
    f32::from_bits(id_bits(murmur3_32(name.as_bytes())))
}

/// The metadata name of `key` (`name`, `hash`, `conversion`, `manifest`) for
/// output `output`: `cryptomatte/<the first seven hex digits of its name's
/// hash>/<key>`.
pub fn metadata(output: &str, key: &str) -> String {
    let hash = format!("{:08x}", murmur3_32(output.as_bytes()));
    format!("cryptomatte/{}/{key}", &hash[..7])
}

/// `attributes` with output `output`'s manifest: those of `names` whose IDs
/// its `ranks` hold (interleaved (ID, coverage) pairs; a pair covering
/// nothing names nothing), in their order, each once.
///
/// # Errors
/// A name a text can't hold.
pub(super) fn manifest(
    attributes: Attributes,
    output: &str,
    names: &[Arc<str>],
    ranks: &[f32],
) -> Result<Attributes, Error> {
    let held: BTreeSet<u32> = (ranks.as_chunks::<2>().0.iter())
        .filter(|pair| pair[1] > 0.0)
        .map(|pair| pair[0].to_bits())
        .collect();
    let mut seen = BTreeSet::new();
    let mut json = String::from("{");
    for name in names {
        let hash = murmur3_32(name.as_bytes());
        if !held.contains(&id_bits(hash)) || !seen.insert(&**name) {
            continue;
        }
        if json.len() > 1 {
            json.push(',');
        }
        quote(name, &mut json);
        json.push_str(&format!(":\"{hash:08x}\""));
    }
    json.push('}');
    let fields = [
        ("name", output.to_owned()),
        ("hash", "MurmurHash3_32".to_owned()),
        ("conversion", "uint32_to_float32".to_owned()),
        ("manifest", json),
    ];
    fields
        .into_iter()
        .try_fold(attributes, |table, (key, value)| {
            let text = Attribute::text(Text::from_strs([value.as_str()]));
            table.with(Domain::Detail, &metadata(output, key), text)
        })
}

/// `text` as a JSON string.
fn quote(text: &str, out: &mut String) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 32 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}
