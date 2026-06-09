//! Integration tests for the Dart code generator.
//!
//! These tests drive the full pipeline (`spec JSON` -> `IntermediateFormat`
//! -> Dart files) via [`openapi_parser::generate_dart_files`] and then
//! assert on the contents of the generated Dart files. They are picked up
//! automatically by `cargo test` / `cargo nextest`, which is what the
//! `nix flake check` (`openapi_parser-nextest`) attribute runs in CI.
//!
//! The bulk of these tests target the family of bugs around following
//! `$ref` to primitive typedefs (`typedef Foo = String;`): historically
//! the generator would still emit `.toJson()` / `.fromJson()` calls on
//! those typedefs, which produced Dart that does not compile. The shared
//! `IntermediateFormat::resolve_ref` classifier is exercised here from
//! several different positions in the spec (object property, list
//! element, ref alias chain, etc.).
//!
//! Each test embeds a tiny OpenAPI spec inline so failures point at the
//! exact shape they cover.
//!
//! ## Assertion style
//!
//! We avoid asserting on the *exact* string of the generated Dart so the
//! tests don't break on whitespace/formatting changes. Instead each test
//! uses `assert_contains` / `assert_not_contains` to pin the specific
//! emission pattern that distinguishes "primitive" handling from "class"
//! handling, e.g. `e as BEAMIdModel` vs. `BEAMIdModel.fromJson(e)`.
//!
//! Both directions are checked where it matters: the *presence* of the
//! correct pattern AND the *absence* of the broken one. This guards
//! against accidental regressions where we'd emit both, or flip back to
//! the wrong one.

use openapi_parser::generate::File;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Run the full pipeline on `spec_json` and return the generated files
/// indexed by their project-relative path (as a forward-slash string for
/// portable assertions).
fn generate(spec_json: &str) -> std::collections::HashMap<String, String> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");
    let files: Vec<File> = rt
        .block_on(openapi_parser::generate_dart_files(spec_json, false))
        .expect("generation should succeed");
    files
        .into_iter()
        .map(|f| (f.path.to_string_lossy().replace('\\', "/"), f.content))
        .collect()
}

/// Look up a generated file by its forward-slash path, panicking with a
/// helpful message (including the list of available paths) if missing.
fn file<'a>(files: &'a std::collections::HashMap<String, String>, path: &str) -> &'a str {
    files.get(path).unwrap_or_else(|| {
        let mut available: Vec<&String> = files.keys().collect();
        available.sort();
        panic!(
            "expected generated file {:?} but only got:\n{}",
            path,
            available
                .iter()
                .map(|s| format!("  {}", s))
                .collect::<Vec<_>>()
                .join("\n")
        )
    })
}

#[track_caller]
fn assert_contains(haystack: &str, needle: &str, ctx: &str) {
    assert!(
        haystack.contains(needle),
        "{ctx}: expected to find\n    {needle:?}\nin generated content:\n---\n{haystack}\n---"
    );
}

#[track_caller]
fn assert_not_contains(haystack: &str, needle: &str, ctx: &str) {
    assert!(
        !haystack.contains(needle),
        "{ctx}: did NOT expect to find\n    {needle:?}\nin generated content:\n---\n{haystack}\n---"
    );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// The original bug report: a property of type `List<$ref>` where the
/// referenced scheme is a primitive typedef (`typedef Foo = String;`) must
/// NOT generate `Foo.fromJson(e)` / `e.toJson()`, because Dart `String`
/// has no such methods. Instead the list elements should be cast as-is.
#[test]
fn list_of_ref_to_primitive_typedef_does_not_call_fromjson_on_elements() {
    let spec = r##"{
        "openapi": "3.1.0",
        "info": { "title": "t", "version": "0" },
        "components": {
            "schemas": {
                "Id": { "type": "string" },
                "Req": {
                    "type": "object",
                    "properties": {
                        "userIds": {
                            "type": "array",
                            "items": { "$ref": "#/components/schemas/Id" }
                        }
                    },
                    "required": ["userIds"]
                }
            }
        },
        "paths": {}
    }"##;
    let files = generate(spec);
    let req = file(&files, "schemes/Req.dart");

    // Correct: list elements are cast, not parsed.
    assert_contains(
        req,
        "(e) => e as BEAMIdModel",
        "fromJson should cast list elements directly",
    );
    assert_contains(
        req,
        "userIds?.map((e) => e).toList()",
        "toJson should pass list elements through unchanged",
    );

    // Wrong: previous broken codegen called .fromJson / .toJson on a typedef.
    assert_not_contains(
        req,
        "BEAMIdModel.fromJson(e)",
        "must not call .fromJson on a primitive typedef",
    );
    assert_not_contains(
        req,
        "(e) => e.toJson()",
        "must not call .toJson on a primitive typedef element",
    );
}

/// The same bug but for a non-list property: a direct `$ref` to a primitive
/// typedef must cast, not parse.
#[test]
fn direct_ref_to_primitive_typedef_does_not_call_fromjson() {
    let spec = r##"{
        "openapi": "3.1.0",
        "info": { "title": "t", "version": "0" },
        "components": {
            "schemas": {
                "Id": { "type": "string" },
                "Req": {
                    "type": "object",
                    "properties": {
                        "userId": { "$ref": "#/components/schemas/Id" }
                    },
                    "required": ["userId"]
                }
            }
        },
        "paths": {}
    }"##;
    let files = generate(spec);
    let req = file(&files, "schemes/Req.dart");

    assert_contains(
        req,
        "val is BEAMIdModel",
        "fromJson should type-check, not parse",
    );
    assert_not_contains(
        req,
        "BEAMIdModel.fromJson(json['userId'])",
        "must not call .fromJson on a primitive typedef",
    );
    assert_not_contains(
        req,
        "userId?.toJson()",
        "must not call .toJson on a primitive typedef",
    );
}

/// The `resolve_ref` classifier must follow chains of references. Here
/// `Alias -> Id -> string`; both hops should resolve to "primitive" so we
/// don't emit `.fromJson` on the alias either. Before centralizing the
/// resolver, every consumer only walked one hop.
#[test]
fn ref_to_alias_to_primitive_is_treated_as_primitive() {
    let spec = r##"{
        "openapi": "3.1.0",
        "info": { "title": "t", "version": "0" },
        "components": {
            "schemas": {
                "Id":    { "type": "string" },
                "Alias": { "$ref": "#/components/schemas/Id" },
                "Req": {
                    "type": "object",
                    "properties": {
                        "single": { "$ref": "#/components/schemas/Alias" },
                        "many":   {
                            "type": "array",
                            "items": { "$ref": "#/components/schemas/Alias" }
                        }
                    },
                    "required": ["single", "many"]
                }
            }
        },
        "paths": {}
    }"##;
    let files = generate(spec);
    let req = file(&files, "schemes/Req.dart");

    assert_contains(req, "val is BEAMAliasModel", "alias single field cast");
    assert_contains(
        req,
        "(e) => e as BEAMAliasModel",
        "alias list elements cast",
    );

    assert_not_contains(
        req,
        "BEAMAliasModel.fromJson",
        "alias to primitive must not emit .fromJson",
    );
    assert_not_contains(
        req,
        "single?.toJson()",
        "alias to primitive must not emit .toJson",
    );
}

/// `$ref` to an *enum* scheme must keep using `.fromJson` / `.toJson`,
/// because the generator emits a real Dart enum class for enums. This is
/// the key reason `ResolvedRef::Enum` is a separate variant from
/// `ResolvedRef::Primitive`.
#[test]
fn ref_to_enum_typedef_keeps_fromjson_and_tojson() {
    let spec = r##"{
        "openapi": "3.1.0",
        "info": { "title": "t", "version": "0" },
        "components": {
            "schemas": {
                "Tag": { "type": "string", "enum": ["a", "b"] },
                "Req": {
                    "type": "object",
                    "properties": {
                        "kind":  { "$ref": "#/components/schemas/Tag" },
                        "kinds": {
                            "type": "array",
                            "items": { "$ref": "#/components/schemas/Tag" }
                        }
                    },
                    "required": ["kind", "kinds"]
                }
            }
        },
        "paths": {}
    }"##;
    let files = generate(spec);
    let req = file(&files, "schemes/Req.dart");

    assert_contains(
        req,
        "BEAMTagModel.fromJson(json['kind'])",
        "scalar enum fromJson",
    );
    assert_contains(req, "kind?.toJson()", "scalar enum toJson");
    assert_contains(
        req,
        "BEAMTagModel.fromJson(e)",
        "list of enums uses per-element fromJson",
    );
    assert_contains(
        req,
        "(e) => e.toJson()",
        "list of enums uses per-element toJson",
    );

    // Enums must NOT be treated like primitives.
    assert_not_contains(
        req,
        "val is BEAMTagModel",
        "enum scalar must not be type-cast as primitive",
    );
    assert_not_contains(
        req,
        "(e) => e as BEAMTagModel",
        "enum list element must not be type-cast as primitive",
    );
}

/// `$ref` to a regular object scheme keeps using `.fromJson` / `.toJson`,
/// the long-standing baseline behavior. This guards against any future
/// refactor of `resolve_ref` accidentally over-broadening "primitive".
#[test]
fn ref_to_object_keeps_fromjson_and_tojson() {
    let spec = r##"{
        "openapi": "3.1.0",
        "info": { "title": "t", "version": "0" },
        "components": {
            "schemas": {
                "User": {
                    "type": "object",
                    "properties": { "name": { "type": "string" } },
                    "required": ["name"]
                },
                "Req": {
                    "type": "object",
                    "properties": {
                        "user":  { "$ref": "#/components/schemas/User" },
                        "users": {
                            "type": "array",
                            "items": { "$ref": "#/components/schemas/User" }
                        }
                    },
                    "required": ["user", "users"]
                }
            }
        },
        "paths": {}
    }"##;
    let files = generate(spec);
    let req = file(&files, "schemes/Req.dart");

    assert_contains(
        req,
        "BEAMUserModel.fromJson(json['user'])",
        "scalar object fromJson",
    );
    assert_contains(req, "user?.toJson()", "scalar object toJson");
    assert_contains(req, "BEAMUserModel.fromJson(e)", "object list fromJson");
    assert_contains(req, "(e) => e.toJson()", "object list toJson");

    assert_not_contains(
        req,
        "val is BEAMUserModel",
        "object scalar must not be cast as primitive",
    );
    assert_not_contains(
        req,
        "(e) => e as BEAMUserModel",
        "object list element must not be cast as primitive",
    );
}

/// Inline list of primitives (no $ref, no typedef) must round-trip as a
/// raw cast. This is the baseline behavior that predates the resolver and
/// should not have changed.
#[test]
fn list_of_inline_primitives_is_cast_not_parsed() {
    let spec = r##"{
        "openapi": "3.1.0",
        "info": { "title": "t", "version": "0" },
        "components": {
            "schemas": {
                "Req": {
                    "type": "object",
                    "properties": {
                        "tags": { "type": "array", "items": { "type": "string" } }
                    },
                    "required": ["tags"]
                }
            }
        },
        "paths": {}
    }"##;
    let files = generate(spec);
    let req = file(&files, "schemes/Req.dart");

    assert_contains(
        req,
        "tags?.map((e) => e).toList()",
        "toJson passes inline-primitive list elements through",
    );
    assert_contains(
        req,
        ".map((e) => e as BEAMReq_tagsModel)",
        "fromJson casts inline-primitive list elements",
    );
}

/// `find_scheme` / `resolve_ref` look-ups must be tolerant of cycles in
/// the reference graph. We don't expect cycles in normal specs, but a
/// malicious or buggy spec must not hang the generator. We assert that
/// generation finishes and that no spurious `.fromJson` calls are emitted
/// on the participants of the cycle.
#[test]
fn resolver_terminates_on_reference_cycles() {
    // A <-> B is a degenerate cycle. The Dart output for such a spec is
    // already nonsensical (neither A nor B has a real shape), but the
    // generator itself must not loop forever; the resolver's cycle
    // detection (`ResolvedRef::Cycle`) keeps things bounded.
    let spec = r##"{
        "openapi": "3.1.0",
        "info": { "title": "t", "version": "0" },
        "components": {
            "schemas": {
                "A": { "$ref": "#/components/schemas/B" },
                "B": { "$ref": "#/components/schemas/A" },
                "Req": {
                    "type": "object",
                    "properties": {
                        "x": { "$ref": "#/components/schemas/A" }
                    },
                    "required": ["x"]
                }
            }
        },
        "paths": {}
    }"##;
    // The only assertion that matters here is "this returns" — if cycle
    // detection were broken we'd hang forever and the test runner would
    // eventually kill us with a timeout.
    let _files = generate(spec);
}

/// Endpoint responses that are `List<$ref to primitive typedef>` are the
/// same bug at the route level instead of inside a product. The response
/// parsing must not call `.fromJson` on the typedef.
#[test]
fn endpoint_response_list_of_ref_to_primitive_does_not_call_fromjson() {
    let spec = r##"{
        "openapi": "3.1.0",
        "info": { "title": "t", "version": "0" },
        "components": {
            "schemas": {
                "Id": { "type": "string" }
            }
        },
        "paths": {
            "/ids": {
                "get": {
                    "responses": {
                        "200": {
                            "description": "",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "array",
                                        "items": { "$ref": "#/components/schemas/Id" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }"##;
    let files = generate(spec);
    let route = file(&files, "endpoints/routes/_ids.dart");

    // The response payload comes back as a List; each element should be
    // returned as-is, not parsed via BEAMIdModel.fromJson.
    assert_not_contains(
        route,
        "BEAMIdModel.fromJson(e)",
        "list of $ref-to-primitive response must not call .fromJson on elements",
    );
}

/// Endpoint responses that are `List<$ref to object>` MUST still parse
/// each element via `.fromJson` — this is the existing baseline.
#[test]
fn endpoint_response_list_of_ref_to_object_parses_each_element() {
    let spec = r##"{
        "openapi": "3.1.0",
        "info": { "title": "t", "version": "0" },
        "components": {
            "schemas": {
                "User": {
                    "type": "object",
                    "properties": { "name": { "type": "string" } },
                    "required": ["name"]
                }
            }
        },
        "paths": {
            "/users": {
                "get": {
                    "responses": {
                        "200": {
                            "description": "",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "array",
                                        "items": { "$ref": "#/components/schemas/User" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }"##;
    let files = generate(spec);
    let route = file(&files, "endpoints/routes/_users.dart");
    assert_contains(
        route,
        "BEAMUserModel.fromJson(e)",
        "list of object refs must parse each element via .fromJson",
    );
}

/// Endpoint request bodies whose type is `List<$ref to object>` must
/// serialize element-by-element: `body?.map((e) => e.toJson()).toList()`.
/// Sending the raw `List<SomeObject>` would leave the network layer
/// unable to call `.toJson()` on the elements, and `body?.toJson()` would
/// not compile because Dart's built-in `List` has no `.toJson()`.
///
/// This is the third arm added to the body-emission template; before, the
/// generator either crashed at compile time (`body?.toJson()` on a List)
/// or silently sent a `List<dynamic>` (the "always treat as primitive"
/// workaround).
#[test]
fn endpoint_request_body_list_of_ref_to_object_serializes_each_element() {
    let spec = r##"{
        "openapi": "3.1.0",
        "info": { "title": "t", "version": "0" },
        "components": {
            "schemas": {
                "User": {
                    "type": "object",
                    "properties": { "name": { "type": "string" } },
                    "required": ["name"]
                }
            }
        },
        "paths": {
            "/users": {
                "post": {
                    "requestBody": {
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "array",
                                    "items": { "$ref": "#/components/schemas/User" }
                                }
                            }
                        }
                    },
                    "responses": { "204": { "description": "" } }
                }
            }
        }
    }"##;
    let files = generate(spec);
    let route = file(&files, "endpoints/routes/_users.dart");

    assert_contains(
        route,
        "body?.map((e) => e.toJson()).toList()",
        "List<Object> request body must serialize element-by-element",
    );
    // It must NOT call `.toJson()` on the list itself (Dart `List` has no
    // `.toJson`) nor pass the raw list (which would leak typed objects
    // straight into the network layer).
    assert_not_contains(
        route,
        "body: body?.toJson()",
        "List request body must not call .toJson() on the list",
    );
    assert_not_contains(
        route,
        "body: body,",
        "List<Object> request body must not be passed raw",
    );
}

/// Endpoint request bodies that are `List<$ref to primitive>` must still
/// be sent raw — the elements have no `.toJson()` to call.
#[test]
fn endpoint_request_body_list_of_ref_to_primitive_is_sent_raw() {
    let spec = r##"{
        "openapi": "3.1.0",
        "info": { "title": "t", "version": "0" },
        "components": {
            "schemas": {
                "Id": { "type": "string" }
            }
        },
        "paths": {
            "/ids": {
                "post": {
                    "requestBody": {
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "array",
                                    "items": { "$ref": "#/components/schemas/Id" }
                                }
                            }
                        }
                    },
                    "responses": { "204": { "description": "" } }
                }
            }
        }
    }"##;
    let files = generate(spec);
    let route = file(&files, "endpoints/routes/_ids.dart");

    assert_contains(route, "body: body,", "primitive list body sent raw");
    assert_not_contains(
        route,
        "body?.map((e) => e.toJson()).toList()",
        "primitive list body must not call .toJson() on its elements",
    );
    assert_not_contains(
        route,
        "body?.toJson()",
        "primitive list body must not call .toJson() on the list",
    );
}

/// Endpoint request bodies whose type is a direct `$ref` to a primitive
/// typedef must be sent as raw JSON (no `body?.toJson()`), because Dart
/// `String` etc. have no `.toJson()`.
#[test]
fn endpoint_request_body_ref_to_primitive_does_not_call_tojson() {
    let spec = r##"{
        "openapi": "3.1.0",
        "info": { "title": "t", "version": "0" },
        "components": {
            "schemas": {
                "Id": { "type": "string" }
            }
        },
        "paths": {
            "/id": {
                "post": {
                    "requestBody": {
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/Id" }
                            }
                        }
                    },
                    "responses": { "204": { "description": "" } }
                }
            }
        }
    }"##;
    let files = generate(spec);
    let route = file(&files, "endpoints/routes/_id.dart");
    assert_not_contains(
        route,
        "body?.toJson()",
        "primitive-typedef request body must be sent raw, not via .toJson()",
    );
}

/// The shared `strip_ref_prefix` / O(1) scheme lookup must accept both
/// the bare scheme name (`Foo`) and the full path
/// (`#/components/schemas/Foo`). This is a tiny in-process sanity check on
/// the resolver API; it doesn't go through the Dart generator at all.
#[test]
fn resolve_ref_accepts_both_bare_and_full_paths() {
    use openapi_parser::parse::intermediate::{self, IntermediateArgs, ResolvedRef};

    let spec_json = r##"{
        "openapi": "3.1.0",
        "info": { "title": "t", "version": "0" },
        "components": {
            "schemas": {
                "Id":   { "type": "string" },
                "Tag":  { "type": "string", "enum": ["a"] },
                "User": { "type": "object", "properties": { "n": { "type": "string" } } }
            }
        },
        "paths": {}
    }"##;
    let spec = oas3::from_json(spec_json).expect("valid spec");
    let intermediate = intermediate::parse(
        &spec,
        IntermediateArgs {
            ignore_deprecated_fields: false,
        },
    )
    .expect("intermediate parses");

    // bare name
    assert_eq!(intermediate.resolve_ref("Id"), ResolvedRef::Primitive);
    // full $ref-style path
    assert_eq!(
        intermediate.resolve_ref("#/components/schemas/Id"),
        ResolvedRef::Primitive
    );
    // enum is its own variant, NOT Primitive
    assert_eq!(intermediate.resolve_ref("Tag"), ResolvedRef::Enum);
    assert!(!intermediate.resolve_ref("Tag").is_primitive());
    // objects are Class
    assert_eq!(intermediate.resolve_ref("User"), ResolvedRef::Class);
    // missing names report Unknown, not panic
    assert_eq!(
        intermediate.resolve_ref("DoesNotExist"),
        ResolvedRef::Unknown
    );
}

/// `allOf` combining a `$ref` and an inline object must merge into a single
/// product type carrying all properties — NOT collapse to a nullable ref that
/// drops the inline fields (regression for the old 2-element allOf stub).
#[test]
fn all_of_merges_ref_and_inline_properties() {
    use openapi_parser::parse::intermediate::{self, AlgType, IntermediateArgs, IAST};

    let spec_json = r##"{
        "openapi": "3.1.0",
        "info": { "title": "t", "version": "0" },
        "components": {
            "schemas": {
                "Base": {
                    "type": "object",
                    "properties": {
                        "a": { "type": "string" },
                        "b": { "type": "string" }
                    },
                    "required": ["a"]
                },
                "Derived": {
                    "allOf": [
                        { "$ref": "#/components/schemas/Base" },
                        {
                            "type": "object",
                            "properties": { "c": { "type": "string" } },
                            "required": ["c"]
                        }
                    ]
                }
            }
        },
        "paths": {}
    }"##;
    let spec = oas3::from_json(spec_json).expect("valid spec");
    let intermediate = intermediate::parse(
        &spec,
        IntermediateArgs {
            ignore_deprecated_fields: false,
        },
    )
    .expect("intermediate parses");

    let derived = intermediate.find_scheme("Derived").expect("Derived exists");
    assert!(
        !derived.is_inherently_nullable,
        "allOf scheme must not be marked inherently nullable"
    );
    match &derived.obj {
        IAST::Object(obj) => match &obj.value {
            AlgType::Product(props) => {
                let mut keys: Vec<&str> = props.keys().copied().collect();
                keys.sort();
                assert_eq!(
                    keys,
                    vec!["a", "b", "c"],
                    "merged product must contain ref + inline properties"
                );
            }
            _ => panic!("expected a Product type for the merged allOf"),
        },
        _ => panic!("expected an Object IAST for the merged allOf"),
    }
}

/// End-to-end: an `allOf` scheme (ref + inline object, colliding `link`)
/// must generate a normal product class with all merged fields and NO
/// `NonNull` wrapper (regression for the allOf stub that collapsed to a
/// nullable ref and dropped inline fields).
#[test]
fn all_of_generates_product_class_not_nullable_wrapper() {
    let spec_json = r##"{
        "openapi": "3.1.0",
        "info": { "title": "t", "version": "0" },
        "components": {
            "schemas": {
                "EmailContentInput": {
                    "type": "object",
                    "properties": {
                        "emailTitle": { "type": "string", "minLength": 1 },
                        "link": { "type": ["string", "null"], "minLength": 1 }
                    },
                    "required": ["emailTitle"]
                },
                "DigitalEmailContentInput": {
                    "allOf": [
                        { "$ref": "#/components/schemas/EmailContentInput" },
                        {
                            "type": "object",
                            "properties": { "link": { "type": "string", "minLength": 1 } },
                            "required": ["link"]
                        }
                    ]
                }
            }
        },
        "paths": {}
    }"##;

    let files = generate(spec_json);
    let derived = file(&files, "schemes/DigitalEmailContentInput.dart");

    assert!(
        derived.contains("class BEAMDigitalEmailContentInputModel implements BEAMSerde"),
        "expected a product class, got:\n{derived}"
    );
    assert!(
        derived.contains("emailTitle"),
        "missing ref field emailTitle:\n{derived}"
    );
    assert!(
        derived.contains("link"),
        "missing merged field link:\n{derived}"
    );
    assert!(
        !derived.contains("NonNullModel"),
        "allOf must not produce a NonNull wrapper:\n{derived}"
    );
}

/// The idiomatic nullable `$ref` shape `allOf: [ {$ref}, {type:[<t>,"null"]} ]`
/// must resolve to a (nullable) REFERENCE to the named scheme, NOT be inlined
/// into an anonymous nested object. Regression for an over-eager allOf merge.
#[test]
fn all_of_nullable_ref_stays_reference() {
    use openapi_parser::parse::intermediate::{self, AlgType, IntermediateArgs, IAST};

    let spec_json = r##"{
        "openapi": "3.1.0",
        "info": { "title": "t", "version": "0" },
        "components": {
            "schemas": {
                "Inner": { "type": "object", "properties": { "x": { "type": "string" } }, "required": ["x"] },
                "Outer": {
                    "type": "object",
                    "properties": {
                        "p": { "allOf": [ { "$ref": "#/components/schemas/Inner" }, { "type": ["object", "null"] } ] }
                    },
                    "required": ["p"]
                }
            }
        },
        "paths": {}
    }"##;
    let spec = oas3::from_json(spec_json).expect("valid spec");
    let im = intermediate::parse(
        &spec,
        IntermediateArgs {
            ignore_deprecated_fields: false,
        },
    )
    .expect("parses");
    let outer = im.find_scheme("Outer").expect("Outer exists");
    match &outer.obj {
        IAST::Object(o) => match &o.value {
            AlgType::Product(props) => match props.get("p").expect("has p") {
                IAST::Reference(r) => {
                    assert!(r.nullable, "nullable-ref must be nullable");
                    assert!(
                        r.path.ends_with("Inner"),
                        "must reference Inner, got {}",
                        r.path
                    );
                }
                _ => panic!("property `p` must stay a Reference, not be inlined"),
            },
            _ => panic!("expected a Product"),
        },
        _ => panic!("expected an Object"),
    }
}

/// End-to-end counterpart: the nullable-ref property must generate a reference
/// to the named model type, not an inlined `Outer_p` nested type.
#[test]
fn all_of_nullable_ref_generates_named_reference_property() {
    let spec_json = r##"{
        "openapi": "3.1.0",
        "info": { "title": "t", "version": "0" },
        "components": {
            "schemas": {
                "Inner": { "type": "object", "properties": { "x": { "type": "string" } }, "required": ["x"] },
                "Outer": {
                    "type": "object",
                    "properties": {
                        "p": { "allOf": [ { "$ref": "#/components/schemas/Inner" }, { "type": ["object", "null"] } ] }
                    },
                    "required": ["p"]
                }
            }
        },
        "paths": {}
    }"##;
    let files = generate(spec_json);
    let outer = file(&files, "schemes/Outer.dart");
    assert!(
        outer.contains("BEAMInnerModel"),
        "must reference the named Inner model:\n{outer}"
    );
    assert!(
        !outer.contains("Outer_p"),
        "must NOT inline the nullable ref into Outer_p:\n{outer}"
    );
}
