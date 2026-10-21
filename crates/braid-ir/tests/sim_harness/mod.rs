//! The shared harness for Braid's seeded codec simulations.
//!
//! Every `sim_*` test file includes this module and adds one line per band, so
//! the `#[test]` attribute is written where a reader and a source-level count
//! both see it. The harness holds the stream, the value generator and the six
//! properties; a band test only names its band and its property.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use braid_ir::{Value, decode_strict, encode};

/// Seeds per band.
pub(crate) const SEEDS: u64 = 64;

/// splitmix64's increment: the 64-bit golden ratio.
const GAMMA: u64 = 0x9e37_79b9_7f4a_7c15;

/// The characters a drawn text is built from, control codes included.
const ALPHABET: [char; 10] = ['a', 'Z', '0', ' ', '\n', '\t', '\\', '"', 'é', '約'];

/// Why a sweep could not draw.
#[derive(Debug)]
pub(crate) enum SimError {
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
pub(crate) struct Stream(u64);

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

/// What a band test calls: a property, over a band of seeds.
pub(crate) type Property = fn(u64) -> Result<(), String>;

/// Every property the harness holds, in the order the six files list them.
///
/// Each `sim_*` binary also links the five properties it does not run; naming
/// them in one table keeps them reachable rather than dead in that binary.
const PROPERTIES: [Property; 6] = [
    roundtrip,
    stable,
    flipped,
    replay,
    text_roundtrip,
    list_roundtrip,
];

/// Runs `property` over every seed of `band`, reporting every failing seed.
pub(crate) fn band(band: u64, property: Property) {
    debug_assert_eq!(
        PROPERTIES.len(),
        6,
        "the table has one entry per simulation file"
    );
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
pub(crate) fn roundtrip(seed: u64) -> Result<(), String> {
    let mut stream = Stream::from_seed(seed);
    let drawn = value(&mut stream, 3).map_err(|error| error.to_string())?;
    match decode_strict(&encode(&drawn)) {
        Ok(read) if read == drawn => Ok(()),
        Ok(read) => Err(format!("read back {read:?}, drew {drawn:?}")),
        Err(error) => Err(format!("refused its own encoding: {error}")),
    }
}

/// An accepted encoding re-encodes to the same bytes: the form is canonical.
pub(crate) fn stable(seed: u64) -> Result<(), String> {
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
pub(crate) fn flipped(seed: u64) -> Result<(), String> {
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
pub(crate) fn replay(seed: u64) -> Result<(), String> {
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
pub(crate) fn text_roundtrip(seed: u64) -> Result<(), String> {
    let mut stream = Stream::from_seed(seed);
    let drawn = Value::Text(text(&mut stream).map_err(|error| error.to_string())?);
    match decode_strict(&encode(&drawn)) {
        Ok(read) if read == drawn => Ok(()),
        Ok(read) => Err(format!("text came back as {read:?}")),
        Err(error) => Err(format!("text refused: {error}")),
    }
}

/// A nested list round-trips to the same depth and order.
pub(crate) fn list_roundtrip(seed: u64) -> Result<(), String> {
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
