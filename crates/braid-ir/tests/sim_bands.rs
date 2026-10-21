//! Seeded sweeps over the canonical codec (`braid-ir`), in the estate's
//! simulation convention: a `sim_*` binary, one stream per seed, no clock, no
//! network, no unseeded draw.
//!
//! Six properties, each over bands of 64 seeds. A seed draws a value, a byte
//! string and a flip position, so a failure names the seed that replays it.
//! The properties are the contract the A4.8 decoder claims: encode then decode
//! is identity, accepted bytes re-encode to themselves, a flipped bit never
//! decodes back to the value it came from, and every draw is a function of its
//! seed alone.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use braid_ir::{Value, decode_strict, encode};

/// Seeds per band.
const SEEDS: u64 = 64;

/// splitmix64's increment: the 64-bit golden ratio.
const GAMMA: u64 = 0x9e37_79b9_7f4a_7c15;

/// The characters a drawn text is built from, control codes included.
const ALPHABET: [char; 10] = ['a', 'Z', '0', ' ', '\n', '\t', '\\', '"', 'é', '約'];

/// Why a sweep could not draw.
#[derive(Debug)]
enum SimError {
    /// A draw was asked for a range with no value in it.
    EmptyRange,
    /// A drawn value did not fit the type it was being narrowed into.
    OutOfRange,
}

impl Display for SimError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match *self {
            Self::EmptyRange => formatter.write_str("a draw below zero has no value"),
            Self::OutOfRange => formatter.write_str("a drawn value did not fit its target type"),
        }
    }
}

impl Error for SimError {}

/// One deterministic stream: splitmix64 over a counter.
struct Stream(u64);

impl Stream {
    /// The stream `seed` names.
    fn from_seed(seed: u64) -> Self {
        Self(seed)
    }

    /// The next 64-bit word.
    fn word(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(GAMMA);
        let mut mixed = self.0;
        mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        mixed ^ (mixed >> 31)
    }

    /// A value in `0..bound`; a bound of zero has none.
    fn below(&mut self, bound: u64) -> Result<u64, SimError> {
        if bound == 0 {
            return Err(SimError::EmptyRange);
        }
        Ok(self.word() % bound)
    }

    /// A value in `0..bound` narrowed to `usize`.
    fn index(&mut self, bound: usize) -> Result<usize, SimError> {
        let wide = u64::try_from(bound).map_err(|_| SimError::OutOfRange)?;
        let drawn = self.below(wide)?;
        usize::try_from(drawn).map_err(|_| SimError::OutOfRange)
    }

    /// A byte string of `len` bytes.
    fn bytes(&mut self, len: usize) -> Result<Vec<u8>, SimError> {
        let mut out = Vec::with_capacity(len);
        for _ in 0..len {
            let drawn = self.below(256)?;
            out.push(u8::try_from(drawn).map_err(|_| SimError::OutOfRange)?);
        }
        Ok(out)
    }
}

/// A text value drawn from the alphabet.
fn text(stream: &mut Stream) -> Result<String, SimError> {
    let len = stream.index(12)?;
    let mut out = String::new();
    for _ in 0..len {
        let index = stream.index(ALPHABET.len())?;
        match ALPHABET.get(index) {
            Some(chosen) => out.push(*chosen),
            None => return Err(SimError::OutOfRange),
        }
    }
    Ok(out)
}

/// An integer drawn from a word.
fn integer(stream: &mut Stream) -> i64 {
    i64::from_ne_bytes(stream.word().to_ne_bytes())
}

/// A value drawn to `depth`, or a leaf at depth zero.
fn value(stream: &mut Stream, depth: u32) -> Result<Value, SimError> {
    let leaf_only = depth == 0;
    let choice = if leaf_only {
        stream.below(4)?
    } else {
        stream.below(6)?
    };
    match choice {
        0 => Ok(Value::Bool(stream.below(2)? == 0)),
        1 => Ok(Value::Int(integer(stream))),
        2 => Ok(Value::Bytes(stream.bytes(8)?)),
        3 => Ok(Value::Text(text(stream)?)),
        4 => {
            let count = stream.below(4)?;
            let mut items = Vec::new();
            for _ in 0..count {
                items.push(value(stream, depth.saturating_sub(1))?);
            }
            Ok(Value::List(items))
        }
        _ => {
            let count = stream.below(4)?;
            let mut map = BTreeMap::new();
            for index in 0..count {
                map.insert(format!("k{index}"), value(stream, depth.saturating_sub(1))?);
            }
            Ok(Value::Map(map))
        }
    }
}

/// Runs `property` over every seed of `band`, reporting every failing seed.
fn over_band(band: u64, property: fn(u64) -> Result<(), String>) {
    let mut failures: Vec<String> = Vec::new();
    for index in 0..SEEDS {
        let seed = band.saturating_mul(SEEDS).saturating_add(index);
        if let Err(report) = property(seed) {
            failures.push(format!("seed {seed}: {report}"));
        }
    }
    assert!(
        failures.is_empty(),
        "{} failing seed(s): {}",
        failures.len(),
        failures.join("; ")
    );
}

/// encode then decode_strict is identity.
fn roundtrip(seed: u64) -> Result<(), String> {
    let mut stream = Stream::from_seed(seed);
    let drawn = value(&mut stream, 3).map_err(|error| error.to_string())?;
    match decode_strict(&encode(&drawn)) {
        Ok(read) if read == drawn => Ok(()),
        Ok(read) => Err(format!("read back {read:?}, drew {drawn:?}")),
        Err(error) => Err(format!("refused its own encoding: {error}")),
    }
}

/// An accepted encoding re-encodes to the same bytes: the form is canonical.
fn stable(seed: u64) -> Result<(), String> {
    let mut stream = Stream::from_seed(seed);
    let drawn = value(&mut stream, 3).map_err(|error| error.to_string())?;
    let bytes = encode(&drawn);
    let read = match decode_strict(&bytes) {
        Ok(read) => read,
        Err(error) => return Err(format!("refused its own encoding: {error}")),
    };
    if encode(&read) == bytes {
        Ok(())
    } else {
        Err(format!("re-encoding moved {} bytes", bytes.len()))
    }
}

/// A flipped bit never decodes back to the value it came from.
fn flipped(seed: u64) -> Result<(), String> {
    let mut stream = Stream::from_seed(seed);
    let drawn = value(&mut stream, 2).map_err(|error| error.to_string())?;
    let mut bytes = encode(&drawn);
    if bytes.is_empty() {
        return Ok(());
    }
    let index = stream
        .index(bytes.len())
        .map_err(|error| error.to_string())?;
    let bit = stream.index(8).map_err(|error| error.to_string())?;
    let shift = u32::try_from(bit).map_err(|error| error.to_string())?;
    if let Some(byte) = bytes.get_mut(index) {
        *byte ^= 1 << shift;
    }
    match decode_strict(&bytes) {
        Ok(read) if read == drawn => Err(format!("bit {bit} of byte {index} changed nothing")),
        _ => Ok(()),
    }
}

/// One seed draws one value, twice.
fn replay(seed: u64) -> Result<(), String> {
    let mut first = Stream::from_seed(seed);
    let mut second = Stream::from_seed(seed);
    let a = value(&mut first, 3).map_err(|error| error.to_string())?;
    let b = value(&mut second, 3).map_err(|error| error.to_string())?;
    if a == b {
        Ok(())
    } else {
        Err(format!("seed replayed two values: {a:?} and {b:?}"))
    }
}

/// Text with control characters and non-ASCII round-trips byte for byte.
fn text_roundtrip(seed: u64) -> Result<(), String> {
    let mut stream = Stream::from_seed(seed);
    let drawn = Value::Text(text(&mut stream).map_err(|error| error.to_string())?);
    match decode_strict(&encode(&drawn)) {
        Ok(read) if read == drawn => Ok(()),
        Ok(read) => Err(format!("text came back as {read:?}")),
        Err(error) => Err(format!("text refused: {error}")),
    }
}

/// A nested list round-trips to the same depth and order.
fn list_roundtrip(seed: u64) -> Result<(), String> {
    let mut stream = Stream::from_seed(seed);
    let depth = stream.below(6).map_err(|error| error.to_string())?;
    let mut drawn = Value::Int(1);
    for _ in 0..depth.saturating_add(1) {
        drawn = Value::List(vec![drawn]);
    }
    match decode_strict(&encode(&drawn)) {
        Ok(read) if read == drawn => Ok(()),
        Ok(read) => Err(format!("nesting came back as {read:?}")),
        Err(error) => Err(format!("nesting refused: {error}")),
    }
}

/// Declares one property's bands: `(test-name, band)` pairs.
macro_rules! bands {
    ($property:ident; $(($name:ident, $band:literal)),+ $(,)?) => {
        $(
            #[test]
            fn $name() {
                over_band($band, $property);
            }
        )+
    };
}

bands!(roundtrip;
    (sim_roundtrip_00, 0), (sim_roundtrip_01, 1), (sim_roundtrip_02, 2), (sim_roundtrip_03, 3), (sim_roundtrip_04, 4), (sim_roundtrip_05, 5),
    (sim_roundtrip_06, 6), (sim_roundtrip_07, 7), (sim_roundtrip_08, 8), (sim_roundtrip_09, 9), (sim_roundtrip_10, 10), (sim_roundtrip_11, 11),
    (sim_roundtrip_12, 12), (sim_roundtrip_13, 13), (sim_roundtrip_14, 14), (sim_roundtrip_15, 15), (sim_roundtrip_16, 16), (sim_roundtrip_17, 17),
    (sim_roundtrip_18, 18), (sim_roundtrip_19, 19), (sim_roundtrip_20, 20), (sim_roundtrip_21, 21), (sim_roundtrip_22, 22), (sim_roundtrip_23, 23),
    (sim_roundtrip_24, 24), (sim_roundtrip_25, 25), (sim_roundtrip_26, 26), (sim_roundtrip_27, 27), (sim_roundtrip_28, 28), (sim_roundtrip_29, 29),
    (sim_roundtrip_30, 30), (sim_roundtrip_31, 31), (sim_roundtrip_32, 32), (sim_roundtrip_33, 33), (sim_roundtrip_34, 34), (sim_roundtrip_35, 35),
    (sim_roundtrip_36, 36), (sim_roundtrip_37, 37), (sim_roundtrip_38, 38), (sim_roundtrip_39, 39), (sim_roundtrip_40, 40), (sim_roundtrip_41, 41),
    (sim_roundtrip_42, 42), (sim_roundtrip_43, 43), (sim_roundtrip_44, 44), (sim_roundtrip_45, 45), (sim_roundtrip_46, 46), (sim_roundtrip_47, 47),
    (sim_roundtrip_48, 48), (sim_roundtrip_49, 49), (sim_roundtrip_50, 50), (sim_roundtrip_51, 51), (sim_roundtrip_52, 52), (sim_roundtrip_53, 53),
    (sim_roundtrip_54, 54), (sim_roundtrip_55, 55), (sim_roundtrip_56, 56), (sim_roundtrip_57, 57), (sim_roundtrip_58, 58), (sim_roundtrip_59, 59),
    (sim_roundtrip_60, 60), (sim_roundtrip_61, 61), (sim_roundtrip_62, 62), (sim_roundtrip_63, 63), (sim_roundtrip_64, 64), (sim_roundtrip_65, 65),
    (sim_roundtrip_66, 66), (sim_roundtrip_67, 67), (sim_roundtrip_68, 68), (sim_roundtrip_69, 69), (sim_roundtrip_70, 70), (sim_roundtrip_71, 71),
);

bands!(stable;
    (sim_stable_00, 0), (sim_stable_01, 1), (sim_stable_02, 2), (sim_stable_03, 3), (sim_stable_04, 4), (sim_stable_05, 5),
    (sim_stable_06, 6), (sim_stable_07, 7), (sim_stable_08, 8), (sim_stable_09, 9), (sim_stable_10, 10), (sim_stable_11, 11),
    (sim_stable_12, 12), (sim_stable_13, 13), (sim_stable_14, 14), (sim_stable_15, 15), (sim_stable_16, 16), (sim_stable_17, 17),
    (sim_stable_18, 18), (sim_stable_19, 19), (sim_stable_20, 20), (sim_stable_21, 21), (sim_stable_22, 22), (sim_stable_23, 23),
    (sim_stable_24, 24), (sim_stable_25, 25), (sim_stable_26, 26), (sim_stable_27, 27), (sim_stable_28, 28), (sim_stable_29, 29),
    (sim_stable_30, 30), (sim_stable_31, 31), (sim_stable_32, 32), (sim_stable_33, 33), (sim_stable_34, 34), (sim_stable_35, 35),
    (sim_stable_36, 36), (sim_stable_37, 37), (sim_stable_38, 38), (sim_stable_39, 39), (sim_stable_40, 40), (sim_stable_41, 41),
    (sim_stable_42, 42), (sim_stable_43, 43), (sim_stable_44, 44), (sim_stable_45, 45), (sim_stable_46, 46), (sim_stable_47, 47),
    (sim_stable_48, 48), (sim_stable_49, 49), (sim_stable_50, 50), (sim_stable_51, 51), (sim_stable_52, 52), (sim_stable_53, 53),
    (sim_stable_54, 54), (sim_stable_55, 55), (sim_stable_56, 56), (sim_stable_57, 57), (sim_stable_58, 58), (sim_stable_59, 59),
    (sim_stable_60, 60), (sim_stable_61, 61), (sim_stable_62, 62), (sim_stable_63, 63), (sim_stable_64, 64), (sim_stable_65, 65),
    (sim_stable_66, 66), (sim_stable_67, 67), (sim_stable_68, 68), (sim_stable_69, 69), (sim_stable_70, 70), (sim_stable_71, 71),
);

bands!(flipped;
    (sim_flipped_00, 0), (sim_flipped_01, 1), (sim_flipped_02, 2), (sim_flipped_03, 3), (sim_flipped_04, 4), (sim_flipped_05, 5),
    (sim_flipped_06, 6), (sim_flipped_07, 7), (sim_flipped_08, 8), (sim_flipped_09, 9), (sim_flipped_10, 10), (sim_flipped_11, 11),
    (sim_flipped_12, 12), (sim_flipped_13, 13), (sim_flipped_14, 14), (sim_flipped_15, 15), (sim_flipped_16, 16), (sim_flipped_17, 17),
    (sim_flipped_18, 18), (sim_flipped_19, 19), (sim_flipped_20, 20), (sim_flipped_21, 21), (sim_flipped_22, 22), (sim_flipped_23, 23),
    (sim_flipped_24, 24), (sim_flipped_25, 25), (sim_flipped_26, 26), (sim_flipped_27, 27), (sim_flipped_28, 28), (sim_flipped_29, 29),
    (sim_flipped_30, 30), (sim_flipped_31, 31), (sim_flipped_32, 32), (sim_flipped_33, 33), (sim_flipped_34, 34), (sim_flipped_35, 35),
    (sim_flipped_36, 36), (sim_flipped_37, 37), (sim_flipped_38, 38), (sim_flipped_39, 39), (sim_flipped_40, 40), (sim_flipped_41, 41),
    (sim_flipped_42, 42), (sim_flipped_43, 43), (sim_flipped_44, 44), (sim_flipped_45, 45), (sim_flipped_46, 46), (sim_flipped_47, 47),
    (sim_flipped_48, 48), (sim_flipped_49, 49), (sim_flipped_50, 50), (sim_flipped_51, 51), (sim_flipped_52, 52), (sim_flipped_53, 53),
    (sim_flipped_54, 54), (sim_flipped_55, 55), (sim_flipped_56, 56), (sim_flipped_57, 57), (sim_flipped_58, 58), (sim_flipped_59, 59),
    (sim_flipped_60, 60), (sim_flipped_61, 61), (sim_flipped_62, 62), (sim_flipped_63, 63), (sim_flipped_64, 64), (sim_flipped_65, 65),
    (sim_flipped_66, 66), (sim_flipped_67, 67), (sim_flipped_68, 68), (sim_flipped_69, 69), (sim_flipped_70, 70), (sim_flipped_71, 71),
);

bands!(replay;
    (sim_replay_00, 0), (sim_replay_01, 1), (sim_replay_02, 2), (sim_replay_03, 3), (sim_replay_04, 4), (sim_replay_05, 5),
    (sim_replay_06, 6), (sim_replay_07, 7), (sim_replay_08, 8), (sim_replay_09, 9), (sim_replay_10, 10), (sim_replay_11, 11),
    (sim_replay_12, 12), (sim_replay_13, 13), (sim_replay_14, 14), (sim_replay_15, 15), (sim_replay_16, 16), (sim_replay_17, 17),
    (sim_replay_18, 18), (sim_replay_19, 19), (sim_replay_20, 20), (sim_replay_21, 21), (sim_replay_22, 22), (sim_replay_23, 23),
    (sim_replay_24, 24), (sim_replay_25, 25), (sim_replay_26, 26), (sim_replay_27, 27), (sim_replay_28, 28), (sim_replay_29, 29),
    (sim_replay_30, 30), (sim_replay_31, 31), (sim_replay_32, 32), (sim_replay_33, 33), (sim_replay_34, 34), (sim_replay_35, 35),
    (sim_replay_36, 36), (sim_replay_37, 37), (sim_replay_38, 38), (sim_replay_39, 39), (sim_replay_40, 40), (sim_replay_41, 41),
    (sim_replay_42, 42), (sim_replay_43, 43), (sim_replay_44, 44), (sim_replay_45, 45), (sim_replay_46, 46), (sim_replay_47, 47),
    (sim_replay_48, 48), (sim_replay_49, 49), (sim_replay_50, 50), (sim_replay_51, 51), (sim_replay_52, 52), (sim_replay_53, 53),
    (sim_replay_54, 54), (sim_replay_55, 55), (sim_replay_56, 56), (sim_replay_57, 57), (sim_replay_58, 58), (sim_replay_59, 59),
    (sim_replay_60, 60), (sim_replay_61, 61), (sim_replay_62, 62), (sim_replay_63, 63), (sim_replay_64, 64), (sim_replay_65, 65),
    (sim_replay_66, 66), (sim_replay_67, 67), (sim_replay_68, 68), (sim_replay_69, 69), (sim_replay_70, 70), (sim_replay_71, 71),
);

bands!(text_roundtrip;
    (sim_text_00, 0), (sim_text_01, 1), (sim_text_02, 2), (sim_text_03, 3), (sim_text_04, 4), (sim_text_05, 5),
    (sim_text_06, 6), (sim_text_07, 7), (sim_text_08, 8), (sim_text_09, 9), (sim_text_10, 10), (sim_text_11, 11),
    (sim_text_12, 12), (sim_text_13, 13), (sim_text_14, 14), (sim_text_15, 15), (sim_text_16, 16), (sim_text_17, 17),
    (sim_text_18, 18), (sim_text_19, 19), (sim_text_20, 20), (sim_text_21, 21), (sim_text_22, 22), (sim_text_23, 23),
    (sim_text_24, 24), (sim_text_25, 25), (sim_text_26, 26), (sim_text_27, 27), (sim_text_28, 28), (sim_text_29, 29),
    (sim_text_30, 30), (sim_text_31, 31), (sim_text_32, 32), (sim_text_33, 33), (sim_text_34, 34), (sim_text_35, 35),
    (sim_text_36, 36), (sim_text_37, 37), (sim_text_38, 38), (sim_text_39, 39), (sim_text_40, 40), (sim_text_41, 41),
    (sim_text_42, 42), (sim_text_43, 43), (sim_text_44, 44), (sim_text_45, 45), (sim_text_46, 46), (sim_text_47, 47),
    (sim_text_48, 48), (sim_text_49, 49), (sim_text_50, 50), (sim_text_51, 51), (sim_text_52, 52), (sim_text_53, 53),
    (sim_text_54, 54), (sim_text_55, 55), (sim_text_56, 56), (sim_text_57, 57), (sim_text_58, 58), (sim_text_59, 59),
    (sim_text_60, 60), (sim_text_61, 61), (sim_text_62, 62), (sim_text_63, 63), (sim_text_64, 64), (sim_text_65, 65),
    (sim_text_66, 66), (sim_text_67, 67), (sim_text_68, 68), (sim_text_69, 69), (sim_text_70, 70), (sim_text_71, 71),
);

bands!(list_roundtrip;
    (sim_list_00, 0), (sim_list_01, 1), (sim_list_02, 2), (sim_list_03, 3), (sim_list_04, 4), (sim_list_05, 5),
    (sim_list_06, 6), (sim_list_07, 7), (sim_list_08, 8), (sim_list_09, 9), (sim_list_10, 10), (sim_list_11, 11),
    (sim_list_12, 12), (sim_list_13, 13), (sim_list_14, 14), (sim_list_15, 15), (sim_list_16, 16), (sim_list_17, 17),
    (sim_list_18, 18), (sim_list_19, 19), (sim_list_20, 20), (sim_list_21, 21), (sim_list_22, 22), (sim_list_23, 23),
    (sim_list_24, 24), (sim_list_25, 25), (sim_list_26, 26), (sim_list_27, 27), (sim_list_28, 28), (sim_list_29, 29),
    (sim_list_30, 30), (sim_list_31, 31), (sim_list_32, 32), (sim_list_33, 33), (sim_list_34, 34), (sim_list_35, 35),
    (sim_list_36, 36), (sim_list_37, 37), (sim_list_38, 38), (sim_list_39, 39), (sim_list_40, 40), (sim_list_41, 41),
    (sim_list_42, 42), (sim_list_43, 43), (sim_list_44, 44), (sim_list_45, 45), (sim_list_46, 46), (sim_list_47, 47),
    (sim_list_48, 48), (sim_list_49, 49), (sim_list_50, 50), (sim_list_51, 51), (sim_list_52, 52), (sim_list_53, 53),
    (sim_list_54, 54), (sim_list_55, 55), (sim_list_56, 56), (sim_list_57, 57), (sim_list_58, 58), (sim_list_59, 59),
    (sim_list_60, 60), (sim_list_61, 61), (sim_list_62, 62), (sim_list_63, 63), (sim_list_64, 64), (sim_list_65, 65),
    (sim_list_66, 66), (sim_list_67, 67), (sim_list_68, 68), (sim_list_69, 69), (sim_list_70, 70), (sim_list_71, 71),
);
