# Braid DSL v0 source subset

**Status:** implementation contract for Braid #77  
**Authority:** D6, D9, D21, ADR-100, `BRAID-DSL-STATE-AND-SUBSTRATE.md`, and `BRAID-GRAPH-DSL.md`  
**Wire:** none. This source always lowers to the existing canonical `Capsule` bytes.
**Decision:** ADR-102 / D33 unlock only this bounded capsule-graph grammar.

## Job

Let a human or agent author the Day-0 CMS reference capsules without Rust,
JSON-of-IR, or JavaScript while preserving the one independent admission path.

## Grammar

```text
document      = "capsule" path "version" integer "{" header* step "}" EOF ;
header        = intent | registry | require | budget | confirm | evidence ;
intent        = "intent" string ";" ;
registry      = "registry" "cms" "::" "v1" ";" ;
require       = "require" "{" capabilities effects "}" ;
capabilities  = "capabilities" "[" path-list? "]" ";" ;
effects       = "effects" "[" ident-list? "]" ";" ;
budget        = "budget" integer ";" ;
confirm       = "confirm" ("none" | "human") ";" ;
evidence      = "evidence" "[" string-list? "]" ";" ;
step          = "step" identifier "{" statement* output "}" ;
statement     = identifier "=" expression ";" ;
expression    = call ("|>" call)* | identifier "|>" call ("|>" call)* ;
call          = path "(" identifier-list? ")" ;
output        = "output" "[" identifier-list "]" ";" ;
path          = identifier ("::" identifier)+ ;
```

Namespaced term and capability paths lower by replacing `::` with `.`. For
example, `cms::edit_section(entity, text)` resolves only if
`cms.edit_section` exists in the pinned CMS registry.

The source capsule name is a diagnostic and project label. It does not enter
the canonical Capsule v0 wire because that wire has no name field; changing
only this label therefore cannot change the CID.

A pipeline prepends the previous result to the next call's explicit arguments:

```text
text |> view::section()
```

is identical to `view::section(text)`.

## Semantic invariants

- Exactly one registry is supported in v0: `cms::v1`. Its registry CID and
  vocabulary version come from `braid-vocab-cms`, never source text.
- `require.capabilities` and `require.effects` are exact assertions over the
  derived capsule. Extra or missing entries are errors.
- Grants are still produced by `braid-sdk::Builder`; source declarations cannot
  create authority.
- Every binding references only earlier bindings. Shadowing, forward references,
  duplicate headers, duplicate requirements, and unused output names are errors.
- The final artifact is admitted by `braid-verify` from canonical bytes. The
  elaborator cannot return success for a verifier rejection.
- Source is bounded to 65,536 bytes, 4,096 tokens, 1,024 bindings, 64 expression
  pipeline stages, 128-byte identifiers, and 4,096-byte strings.

## Explicit refusals

The v0 parser refuses `schema`, `state`, `statechart`, `orchestration`, imports,
macros, arbitrary loops, recurrence, floats, runtime literal expressions, raw
URLs, embedded code, and any term outside the closed CMS vocabulary. These are
not silently ignored or represented as intent text.

`lit::text()`, `lit::bytes()`, and `lit::entity()` are existing zero-input
vocabulary terms. They do not carry source literal payloads. Payload syntax
therefore remains forbidden until the canonical Strand payload contract lands.

## CLI contract

```text
braid dsl compile <source.brd> -o <capsule.braid> [--emit-json <capsule.json>]
braid dsl check <source.brd>
```

`compile` writes nothing until parsing, lowering, canonical encoding, and
independent admission all succeed. `check` performs the same work without
writing artifacts. Both print the CID and manifest receipt.

## Acceptance probes

1. The three demo-port DSL fixtures produce bytes identical to their existing
   JSON-of-IR fixtures and retain the pinned CIDs.
2. Ten golden programs pin exact CIDs; ten invalid programs pin diagnostic
   codes and fail without artifacts.
3. Unknown terms, widened capabilities, missing effects, unconfirmed publish,
   excessive depth/work, and non-canonical output attempts fail closed.
4. A source change that adds `cms::publish` is visible as a manifest widening.
