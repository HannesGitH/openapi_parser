# openapi spec parser

takes in a open api spec json and generates rigorous client code, better than the original

## supported languages

- dart

## usage

```sh
openapi_parser -u $spec_url -o $out_dir -d $target_lang
```

### options

- `--unmodifiable` (default `true`): emit immutable types. In the default
  (eager) mode this means `final` properties and `const` constructors; in
  `--lazy` mode the backing map is a private `_rawValue`. Pass
  `--unmodifiable false` to expose mutable/public state.
- `--lazy` (default `false`): emit **map-backed lazy** models instead of eager
  typed ones (see below).
- `--skip-deprecated-removal`: keep properties/parameters that reference
  deprecated schemes.

### eager vs lazy models

By default each schema becomes a class with typed `final` fields that are
parsed and validated up front in a `fromJson` factory (fail-fast at the
network boundary).

With `--lazy`, each schema is instead backed by a single `rawValue` map:

- `fromJson` just wraps the decoded map (a `const` constructor under
  `--unmodifiable`); `toJson()` returns it verbatim (unknown keys preserved).
- Properties become getters that parse their slice **on read**, so a malformed
  field only throws (`BEAMWrongTypeError`) if you actually access it — tolerant
  of unexpected backend data you don't consume (Postel's law), and it avoids
  materializing large payloads you only partially read.
- The typed constructor still exists for building outgoing values; it eagerly
  builds the backing map (so it is not `const`).
- List/Map getters return `UnmodifiableListView` / `UnmodifiableMapView`.

Because lazy `fromJson` no longer throws on mismatch, non-discriminated
`oneOf` unions discriminate via a generated
`static bool checkConform(dynamic json, {bool deep = true})` instead of
catching parse failures; it validates deeply by default.

Note: lazy output uses null-aware collection elements and therefore requires a
Dart SDK that supports them (>= 3.8).
