# openapi spec parser

takes in a open api spec json and generates rigorous client code, better than the original

## supported languages

- dart

## usage

```sh
openapi_parser -u $spec_url -o $out_dir -d $target_lang
```

### options

- `--modifiable`: expose mutable/public state. By default types are
  immutable — in the default (eager) mode this means `final` properties
  and `const` constructors; in `--lazy` mode the backing map is a private
  `_rawValue`.
- `--lazy`: emit **map-backed lazy** models instead of eager typed ones
  (see below).
- `--skip-deprecated-removal`: keep properties/parameters that reference
  deprecated schemes.

### eager vs lazy models

By default each schema becomes a class with typed `final` fields that are
parsed and validated up front in a `fromJson` factory (fail-fast at the
network boundary).

With `--lazy`, each schema is instead backed by a single `rawValue` map:

- `fromJson` just wraps the decoded map (a `const` constructor unless
  `--modifiable`); `toJson()` returns it verbatim (unknown keys preserved).
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

### conformity checks

Every generated `BEAMSerde` type (models, enums, unions, multi-status response
unions) has a static `checkConform(json, {deep})`, in both eager and lazy
mode, plus an instance `bool conforms({bool deep = true})` that runs it against
the value's own raw JSON (the backing map for lazy models, `toJson()`
otherwise). Under `--lazy` this tells you whether a model you received
actually matches the spec without touching every getter.

To find out *what* doesn't match, every such type also has a static
`checkConformityErrors(json, {path = r'$', deep = true})` and an instance
`conformityErrors({path, deep})`. They walk the same structure as
`checkConform` but collect every mismatch instead of stopping at the first
(`checkConform` itself stays short-circuiting, since union discrimination
relies on it):

```
BeamConformErr: $.items[3].amount expected num, received String
BeamConformErr: $.frequency expected one of weekly|monthly, received "monthlyy"
BeamConformErr: $.pet expected one of Cat | Dog, received object
```

The result is empty exactly when `checkConform` is true. `received` holds only
the JSON kind of the value, never the value itself, so errors are safe to log.
The one exception is an enum (or discriminator) string, which is shown in
quotes. A plain union reports one error at its own path; a discriminated union
reports the tagged variant's own errors.

### debug hooks

A `JsonRequestHandler` may additionally mix in `BeamHandlerHooks` (opt-in, like
`BeamStatusCodeAwareHandler`; plain handlers keep working unchanged):

```dart
class MyHandler with BeamHandlerHooks implements JsonRequestHandler {
  @override
  T afterDecode<T>({required BEAMPathEnum path, required BEAMRequestMethod method, required T value}) {
    if (value is BEAMSerde && !value.conforms()) log('$method $path: non-conforming response');
    return value;
  }
  // beforeRequest<T>(...) sees the typed request body before `toJson()`.
  ...
}
```

Generated routes call `beforeRequest` once per request that has a body and
`afterDecode` once per decoded value (cached and upstream). Both hooks are
synchronous, and values produced by your own `.then` chains never reach them.
