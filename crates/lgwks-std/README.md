# lgwks-std — the `+` in `std+`

**The rule, in the Director's words:** if a library is not in `std` or `std+`,
it is not an approved dependency, and the code does not compile until a human
has registered it in the semantic contract.

This crate is the `+`. `lgwks-std-gate` is the "does not compile."

## What it provides

Every module is a declarative primitive — one import, one function call, done.
The crate wraps a vetted dependency stack that bottoms out at zero external deps,
so a consumer gets exactly `lgwks_std` in their manifest and nothing else to
think about.

```rust
use lgwks_std::hash;
use lgwks_std::hex;
use lgwks_std::id::Uuid;
use lgwks_std::pattern::Regex;
use lgwks_std::time;

let digest = hash::blake3(b"hello world");
let hex_str = hex::encode(digest.as_bytes());
let id = Uuid::new_v4().unwrap();
let now = time::now_rfc3339();
let re = Regex::new(r"\d+").unwrap();
```

## Modules

| module | what it owns | replaces |
|--------|-------------|----------|
| `hash` | BLAKE3 content-addressable hashing | `blake3` crate direct dep |
| `pattern` | Compiled regex matching (linear-time) | `regex` crate direct dep |
| `hex` | Hex encode/decode | `hex` crate |
| `id` | UUID v4 generation and parsing | `uuid` crate |
| `glob` | Shell-style path pattern matching (DP, O(M×N)) | `glob` crate |
| `time` | RFC 3339 timestamps, calendar math | `chrono` / `time` crate |
| `encoding::base64` | Base64 encode/decode (RFC 4648) | `base64` crate |
| `encoding::percent` | Percent-encoding for URI components | `percent-encoding` crate |
| `random` | OS entropy (`/dev/urandom`) | `getrandom` crate |
| `leb128` | LEB128 variable-length integer encoding | — |
| `fs` | Recursive directory walking with sandbox enforcement | `walkdir` crate |
| `task` | Minimal single-threaded async executor | — |

## INV-STDPLUS-APPROVED-ONLY

The crate depends on exactly two external crates, both vetted leaf stacks:

- **`blake3`** (3 zero-dep leaves: arrayvec, cfg-if, constant_time_eq)
- **`regex`** (4 crates, all BurntSushi, all internal: memchr, regex-syntax,
  aho-corasick, regex-automata — zero external deps)

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
