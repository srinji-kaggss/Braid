# lgwks-std — the `+` in `std+`

**The rule, in the Director's words:** if a library is not in `std` or `std+`,
it is not an approved dependency, and the code does not compile until a human
has registered it in the semantic contract.

This crate is the `+`. `lgwks-std-gate` is the "does not compile."

## Supported Rust policy

`lgwks_std` and `lgwks_std_gate` are governed by two Rust contracts:

- **Current stable contract:** workspace and CI run against Rust 1.98.0.
- **MSRV contract:** `Cargo.toml` `rust-version` fields are the minimum supported compiler, not the
  active CI compiler. They move forward only when code or dependency requirements require it.

## What it provides

Every module is a declarative primitive — one import, one function call, done.
The crate wraps a vetted dependency stack that bottoms out at zero external deps.
Core modules are in the default feature set; heavier stacks are optional features
so path-only consumers do not pull heavyweight transitive dependencies.

```rust
use lgwks_std::hex;
use lgwks_std::id::Uuid;
#[cfg(feature = "hash")]
use lgwks_std::hash;
#[cfg(feature = "pattern")]
use lgwks_std::pattern::Regex;
use lgwks_std::time;

#[cfg(feature = "hash")]
{
let digest = lgwks_std::hash::blake3(b"hello world");
let hex_str = hex::encode(digest.as_bytes());
}
let id = Uuid::new_v4().unwrap();
let now = time::now_rfc3339();
#[cfg(feature = "pattern")]
let re = Regex::new(r"\d+").unwrap();
```

## Modules

Default feature set: `core` (enabled by default).

| feature | module | what it owns | replaces |
|---------|--------|-------------|----------|
| `core` | `encoding` | Encoding helpers and base64/percent primitives | `base64`, `percent-encoding` |
| `core` | `fs` | Recursive directory walking with sandbox enforcement | `walkdir` |
| `core` | `glob` | Shell-style path pattern matching (DP, O(M×N)) | `glob` |
| `core` | `hex` | Hex encode/decode | `hex` |
| `core` | `id` | UUID v4 generation and parsing | `uuid` |
| `core` | `leb128` | LEB128 variable-length integer encoding | — |
| `core` | `random` | OS entropy (`/dev/urandom`) | `getrandom` |
| `core` | `task` | Minimal single-threaded async executor | — |
| `core` | `time` | RFC 3339 timestamps, calendar math | `chrono` / `time` |
| `hash` | `hash` | BLAKE3 content-addressable hashing | `blake3` |
| `pattern` | `pattern` | Compiled regex matching (linear-time) | `regex` |
| `json` | `json` | JSON encoding and decoding | `serde`, `serde_json` |
| `wire` | `wire` | Internal binary wire serialization | `rkyv` |

Feature presets:

- `core` (default): lightweight primitives above.
- `hash`, `pattern`, `json`, `wire`: each unlocks one capability stack.
- `full`: all optional stacks.

Legacy module ownership map:

| module | what it owns |
|--------|-------------|
| `hash` | BLAKE3 content-addressable hashing |
| `pattern` | Compiled regex matching (linear-time) |
| `hex` | Hex encode/decode |
| `id` | UUID v4 generation and parsing |
| `glob` | Shell-style path pattern matching (DP, O(M×N)) |
| `time` | RFC 3339 timestamps, calendar math |
| `encoding::base64` | Base64 encode/decode (RFC 4648) |
| `encoding::percent` | Percent-encoding for URI components |
| `random` | OS entropy (`/dev/urandom`) |
| `leb128` | LEB128 variable-length integer encoding |
| `fs` | Recursive directory walking with sandbox enforcement |
| `task` | Minimal single-threaded async executor |

## INV-STDPLUS-APPROVED-ONLY

The crate declares six external crates, all vetted leaf stacks, enabled by
features:

- **`blake3`** (feature `hash`; 3 zero-dep leaves: arrayvec, cfg-if,
  constant_time_eq)
- **`regex`** (feature `pattern`; 4 crates, all BurntSushi, all internal:
  memchr, regex-syntax, aho-corasick, regex-automata — zero external deps)
- **`serde`** (feature `json`; derive stack shared with serde_json)
- **`serde_json`** (feature `json`)
- **`rkyv`** (feature `wire`; 5 djkoloski crates: rend, ptr_meta, rancor,
  munge + derive; zero external deps)
- **`getrandom`** (default path for `random`; supported on linux/macOS/windows in
  this crate)

The gate test `deps_are_approved_leaves` in `lgwks-std-gate` mechanically
verifies no unapproved dependency appears in the manifest.

## The gate

Three lines in a consumer's `build.rs` turn INV-DEP-REGISTERED into a compile
error:

```rust
// build.rs
fn main() {
    lgwks_std_gate::enforce();
}
```

## License

MPL-2.0
