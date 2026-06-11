pub mod types;
use std::collections::{BTreeMap, HashMap, HashSet};

use oas3::spec::Response as Responses;
use oas3::spec::*;
pub use types::*;
#[macro_use]
mod macros;

#[derive(Debug, Clone)]
pub struct IntermediateArgs {
    pub ignore_deprecated_fields: bool,
}

/// Context carried through the entire intermediate parse. Holds the user args
/// plus a precomputed set of top-level scheme names that are marked
/// `deprecated: true` in `components.schemas`, so we can cheaply decide whether
/// a `$ref` points at a deprecated scheme.
pub struct ParseCtx<'a> {
    pub args: IntermediateArgs,
    pub deprecated_schemes: HashSet<&'a str>,
    pub schemas: &'a BTreeMap<String, ObjectOrReference<ObjectSchema>>,
}

impl<'a> ParseCtx<'a> {
    fn ref_targets_deprecated(&self, ref_path: &str) -> bool {
        // `strip_ref_prefix` is the single shared helper, defined in
        // `types.rs` and re-exported via `pub use types::*;` above.
        self.deprecated_schemes.contains(strip_ref_prefix(ref_path))
    }
}

/// Uniform deprecation check across the three IAST shapes. Used by the
/// property/parameter filters when `ignore_deprecated_fields` is on.
fn iast_is_deprecated(iast: &IAST) -> bool {
    match iast {
        IAST::Object(o) => o.is_deprecated,
        IAST::Primitive(p) => p.is_deprecated,
        IAST::Reference(r) => r.is_deprecated,
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    NoComponents,
    ParseError(String),
}

pub fn parse(spec: &oas3::Spec, args: IntermediateArgs) -> Result<IntermediateFormat<'_>, Error> {
    let mut schemes = Vec::new();
    let components = match &spec.components {
        Some(components) => components,
        None => return Err(Error::NoComponents),
    };

    // Precompute which top-level schemes are deprecated. We KEEP deprecated
    // schemes in the output (option A) so that sum-type variants and ref
    // passthroughs still resolve; the set is only used to decide whether a
    // property/parameter referencing such a scheme should be dropped.
    let deprecated_schemes: HashSet<&str> = components
        .schemas
        .iter()
        .filter_map(|(name, schema)| match schema {
            ObjectOrReference::Object(obj) if obj.deprecated == Some(true) => Some(name.as_str()),
            _ => None,
        })
        .collect();

    let ctx = ParseCtx {
        args,
        deprecated_schemes,
        schemas: &components.schemas,
    };

    for (name, schema) in components.schemas.iter() {
        let obj = match parse_schema(&ctx, schema, false, false) {
            Ok(obj) => obj,
            Err(e) => return Err(e),
        };
        schemes.push(Scheme {
            name: name.as_str(),
            is_inherently_nullable: match &obj {
                IAST::Object(obj) => obj.nullable,
                IAST::Reference(refe) => refe.nullable,
                IAST::Primitive(prim) => prim.nullable,
            },
            obj,
        });
    }

    let routes: Vec<Route> = match &spec.paths {
        Some(paths) => {
            let mut routes = Vec::new();
            for (path, route) in paths.iter() {
                routes.push(Route {
                    path: path.as_str(),
                    description: route.description.as_deref(),
                    endpoints: {
                        let mut endpoints = Vec::new();

                        let parser = macros::EndpointParser {
                            ctx: &ctx,
                            params_parser: &parse_params,
                            request_parser: &parse_request,
                            responses_parser: &parse_responses,
                        };

                        println!("route: {}", path);

                        handle_endpoint!(&parser, &mut endpoints, &route.get, Method::Get);
                        handle_endpoint!(&parser, &mut endpoints, &route.post, Method::Post);
                        handle_endpoint!(&parser, &mut endpoints, &route.put, Method::Put);
                        handle_endpoint!(&parser, &mut endpoints, &route.delete, Method::Delete);
                        handle_endpoint!(&parser, &mut endpoints, &route.patch, Method::Patch);
                        handle_endpoint!(&parser, &mut endpoints, &route.options, Method::Options);
                        handle_endpoint!(&parser, &mut endpoints, &route.head, Method::Head);
                        handle_endpoint!(&parser, &mut endpoints, &route.trace, Method::Trace);

                        endpoints
                    },
                });
            }
            routes
        }
        None => vec![],
    };

    let routes_tree = convert_routes_to_tree(&routes);
    Ok(IntermediateFormat::new(schemes, routes, routes_tree))
}

fn parse_params<'a>(
    ctx: &ParseCtx<'a>,
    params: &'a Vec<ObjectOrReference<Parameter>>,
) -> Result<Vec<Param<'a>>, Error> {
    params
        .iter()
        .map(|param| match param {
            ObjectOrReference::Object(p) => {
                let Parameter {
                    name,
                    location,
                    description,
                    required,
                    deprecated,
                    schema,
                    ..
                } = p;
                if location == &ParameterIn::Path {
                    return None;
                }
                if ctx.args.ignore_deprecated_fields {
                    // skip params explicitly marked deprecated
                    if deprecated.unwrap_or(false) {
                        return None;
                    }
                    // skip params whose schema is a $ref to a deprecated scheme
                    if let Some(ObjectOrReference::Ref { ref_path }) = schema {
                        if ctx.ref_targets_deprecated(ref_path) {
                            return None;
                        }
                    }
                }
                Some(Ok(Param {
                    name: name.as_str(),
                    description: description.as_deref(),
                    required: required.unwrap_or(false),
                }))
            }
            ObjectOrReference::Ref { ref_path, .. } => Some(Err(Error::ParseError(format!(
                "Reference to {} not supported in params",
                ref_path
            )))),
        })
        .filter_map(|param| param)
        .collect()
}

fn parse_request<'a>(
    ctx: &ParseCtx<'a>,
    request: Option<&'a ObjectOrReference<RequestBody>>,
) -> Result<IAST<'a>, Error> {
    match request {
        Some(ObjectOrReference::Object(req_body)) => {
            //we only consider a single possible format (like application/json) for now
            let scheme = req_body
                .content
                .iter()
                .next()
                .unwrap()
                .1
                .schema
                .as_ref()
                .unwrap();
            parse_schema(ctx, scheme, false, false)
        }
        Some(ObjectOrReference::Ref { ref_path, .. }) => Ok(IAST::Reference(AnnotatedReference {
            path: ref_path,
            optional: false,
            nullable: false,
            is_deprecated: ctx.ref_targets_deprecated(ref_path),
        })),
        None => Err(Error::ParseError("No request body".to_string())),
    }
}
fn parse_responses<'a>(
    ctx: &ParseCtx<'a>,
    responses: &'a BTreeMap<String, ObjectOrReference<Responses>>,
) -> Result<BTreeMap<&'a String, IAST<'a>>, Error> {
    let mut map = BTreeMap::new();
    for (code, response) in responses {
        match response {
            ObjectOrReference::Object(req_body) => {
                //we only consider a single possible format (like application/json) for now
                let scheme_opt = &req_body.content.iter().next();
                let scheme = match scheme_opt {
                    Some(schema) => &schema.1.schema,
                    None => return Err(Error::ParseError("No response body".to_string())),
                };
                if let Some(schema) = scheme {
                    let schema = parse_schema(ctx, &schema, false, false).unwrap();
                    map.insert(code, schema);
                }
            }
            ObjectOrReference::Ref { ref_path, .. } => {
                let schema = IAST::Reference(AnnotatedReference {
                    path: ref_path,
                    optional: false,
                    nullable: false,
                    is_deprecated: ctx.ref_targets_deprecated(ref_path),
                });
                map.insert(code, schema);
            }
        }
    }
    Ok(map)
}

fn parse_schema<'a>(
    ctx: &ParseCtx<'a>,
    schema: &'a ObjectOrReference<ObjectSchema>,
    is_optional: bool,
    // only used in rare edge case where we have an object that is only a ref but can also be null
    ref_is_nullable: bool,
) -> Result<IAST<'a>, Error> {
    match schema {
        ObjectOrReference::Object(object) => parse_object(ctx, object, is_optional),
        ObjectOrReference::Ref { ref_path } => Ok(IAST::Reference(AnnotatedReference {
            path: ref_path,
            optional: is_optional,
            nullable: ref_is_nullable,
            is_deprecated: ctx.ref_targets_deprecated(ref_path),
        })),
    }
}

fn parse_object<'a>(
    ctx: &ParseCtx<'a>,
    object: &'a ObjectSchema,
    is_optional: bool,
) -> Result<IAST<'a>, Error> {
    if object.format == Some("binary".to_string()) {
        return Ok(IAST::Primitive(AnnotatedObj {
            nullable: false,
            optional: is_optional,
            is_deprecated: object.deprecated.unwrap_or(false),
            description: object.description.as_deref(),
            title: object.title.as_deref(),
            value: Primitive::Binary,
        }));
    }
    let parse_properties = || {
        // if either the parent said its nullable (by not being required) or itself is nullable
        let is_nullable = object.is_nullable().unwrap_or(false);
        Ok(IAST::Object(AnnotatedObj {
            nullable: is_nullable,
            optional: is_optional,
            is_deprecated: object.deprecated.unwrap_or(false),
            description: object.description.as_deref(),
            title: object.title.as_deref(),
            value: AlgType::Product(
                match object
                    .properties
                    .iter()
                    .filter_map(|(name, schema)| {
                        let is_required =
                            object.required.iter().any(|n| n.as_str() == name.as_str());
                        // println!("parsing property: {}, nullable: {}", name, !is_required);

                        if name.starts_with("dummy") {
                            println!("v1001: {} optional:{:?} {:?}", name, !is_required, schema);
                        }

                        //TODO: there was a case where a required object could either be an object or null, but the oas3 spec properties where empty although the json had some, idk
                        match parse_schema(ctx, schema, !is_required, false) {
                            Ok(obj) => {
                                // Drop properties that are deprecated (either inline, or a $ref
                                // pointing at a deprecated scheme). Piggy-backs on `is_deprecated`
                                // which is set uniformly across IAST variants by parse_schema.
                                if ctx.args.ignore_deprecated_fields && iast_is_deprecated(&obj) {
                                    None
                                } else {
                                    Some(Ok((name.as_str(), obj)))
                                }
                            }
                            Err(e) => Some(Err(e)),
                        }
                    })
                    .collect::<Result<HashMap<_, _>, _>>()
                {
                    Ok(types) => types,
                    Err(e) => return Err(e),
                },
            ),
        }))
    };

    let parse_prim_type = |typ: &SchemaType| {
        // enum_values will be a vector with the possible values, each with a an additional bool, indicating weather it is a string (true) or a native type (false)
        let enum_values = if let Some(const_value) = &object.const_value {
            Some(vec![EnumValue {
                value: const_value.to_string(),
                is_string: const_value.is_string(),
            }])
        } else if !object.enum_values.is_empty() {
            Some(
                object
                    .enum_values
                    .iter()
                    .map(|v| EnumValue {
                        value: v.to_string().trim_matches('"').to_string(),
                        is_string: v.is_string(),
                    })
                    .collect(),
            )
        } else {
            None
        };
        if let Some(mut enum_values) = enum_values {
            if let Some(serde_json::Value::Bool(true)) =
                object.extensions.get("allow-unspecified-values")
            {
                // our parser checks for unspecified, so add it
                enum_values.push(EnumValue {
                    value: "unspecified".to_string(),
                    is_string: true,
                });
            }
            return Primitive::Enum(enum_values);
        }
        match typ {
            SchemaType::Boolean => Primitive::Boolean,
            SchemaType::Integer => Primitive::Integer,
            SchemaType::Number => Primitive::Number,
            SchemaType::String => Primitive::String,
            SchemaType::Null => Primitive::Never,
            //TODO: maybe parse properties here
            SchemaType::Object => match parse_properties() {
                Ok(obj) => Primitive::Map(Box::new(obj)),
                Err(e) => {
                    println!("error parsing object: {:?}", e);
                    Primitive::Dynamic
                }
            },
            SchemaType::Array => match &object.items {
                Some(items) => match parse_schema(ctx, &items, false, false) {
                    Ok(obj) => Primitive::List(Box::new(obj)),
                    Err(e) => {
                        println!("error parsing list: {:?}", e);
                        Primitive::Dynamic
                    }
                },
                None => {
                    println!("error parsing list, no items");
                    Primitive::Dynamic
                }
            },
        }
    };
    // if type is set, we can return a primitive type
    if let Some(types) = &object.schema_type {
        let prim_type = match types {
            SchemaTypeSet::Single(typ) => typ,
            // we currently don't support multiple primitive types
            SchemaTypeSet::Multiple(types) => types
                .iter()
                .filter(|typ| typ != &&SchemaType::Null)
                .collect::<Vec<_>>()
                .first()
                .unwrap_or(&&SchemaType::Null),
        };
        if prim_type != &SchemaType::Object {
            let value = parse_prim_type(prim_type);
            return Ok(IAST::Primitive(AnnotatedObj {
                nullable: types.contains(SchemaType::Null),
                optional: is_optional,
                is_deprecated: object.deprecated.unwrap_or(false),
                description: object.description.as_deref(),
                title: object.title.as_deref(),
                value,
            }));
        }
        // if its an object, we need to parse the properties, so simply continue
    }
    // otherwise we need to parse the object, and return the algebraic type
    // 1: if its has any_of or one_of set, we need to return a sum type
    // 2: if its has properties set, we need to return a product type
    // 3  if it has all of set its probably a nullable ref weird edge case situation

    // 1:
    if !object.any_of.is_empty() || !object.one_of.is_empty() {
        // it cannot have both so we just precede with any_of
        let union_types = if !object.any_of.is_empty() {
            &object.any_of
        } else {
            &object.one_of
        };
        let discrimination = match &object.discriminator {
            Some(Discriminator {
                mapping: Some(mapping),
                property_name,
            }) => Some(Discrimination {
                key: property_name.as_str(),
                // theoretisch können die auch mal kein mapping haben, dann muss die value hinter dem key in dem object den Namen des objects haben
                map: mapping
                    .into_iter()
                    .map(|(k, v)| {
                        (
                            k.as_str(),
                            AnnotatedReference {
                                path: v.as_str(),
                                optional: false,
                                nullable: false,
                                // sum-type variants are KEPT even if deprecated, but we still
                                // surface the flag so codegen can annotate them appropriately
                                is_deprecated: ctx.ref_targets_deprecated(v.as_str()),
                            },
                        )
                    })
                    .collect(),
            }),
            _ => None,
        };
        let nullable = union_types.iter().any(|schema| match schema {
            ObjectOrReference::Object(schema) => {
                schema.is_nullable().unwrap_or(false)
                    || if let Some(SchemaTypeSet::Single(SchemaType::Null)) = schema.schema_type {
                        true
                    } else {
                        false
                    }
            }
            ObjectOrReference::Ref { .. } => false,
        });
        return Ok(IAST::Object(AnnotatedObj {
            nullable,
            optional: is_optional,
            is_deprecated: object.deprecated.unwrap_or(false),
            description: object.description.as_deref(),
            title: object.title.as_deref(),
            value: match union_types
                .iter()
                .enumerate()
                .map(
                    |(idx, schema)| match parse_schema(ctx, schema, false, nullable) {
                        Ok(obj) => Ok(types::SumVariant {
                            name: match &obj {
                                IAST::Reference(refe) => strip_ref_prefix(refe.path).to_string(),
                                _ => idx.to_string(),
                            },
                            typ: obj,
                        }),
                        Err(e) => {
                            return Err(e);
                        }
                    },
                )
                .collect::<Result<Vec<_>, _>>()
            {
                Ok(types) => match discrimination {
                    Some(discrimination) => AlgType::DiscriminatedSum(discrimination),
                    None => AlgType::Sum(types),
                },
                Err(e) => return Err(e),
            },
        }));
    }

    // 2:
    if !object.properties.is_empty() {
        return parse_properties();
    }

    // 3: allOf. Two real shapes occur in practice:
    //   (a) nullable `$ref`:  allOf: [ {$ref}, {type:[<t>,"null"]} ]
    //       The non-ref member only attaches nullability (and/or a description)
    //       to the ref and contributes NO properties. This MUST stay a reference
    //       to the named scheme so generated type names remain stable for
    //       consumers (this is the common case).
    //   (b) composition:      allOf: [ {$ref}, {object with properties}, ... ]
    //       At least one member adds real properties -> merge everything into a
    //       single flat product type (intersection semantics).
    if !object.all_of.is_empty() {
        let all = &object.all_of;

        // Does any member contribute its own properties? Bare `$ref`s and pure
        // nullability/`type` markers (e.g. {"type":["object","null"]}) do not.
        let has_extra_props = all.iter().any(|s| match s {
            ObjectOrReference::Object(o) => !o.properties.is_empty() || !o.all_of.is_empty(),
            ObjectOrReference::Ref { .. } => false,
        });

        // Does any member permit null (the marker member in shape (a))?
        let allows_null = all.iter().any(|s| match s {
            ObjectOrReference::Object(o) => schema_allows_null(o),
            ObjectOrReference::Ref { .. } => false,
        });

        if !has_extra_props {
            // Shape (a): no member adds fields -> a (possibly nullable) reference.
            if let Some(ObjectOrReference::Ref { ref_path }) = all
                .iter()
                .find(|s| matches!(s, ObjectOrReference::Ref { .. }))
            {
                return Ok(IAST::Reference(AnnotatedReference {
                    path: ref_path,
                    optional: is_optional,
                    nullable: allows_null,
                    is_deprecated: ctx.ref_targets_deprecated(ref_path),
                }));
            }
        } else {
            // Shape (b): merge all members (referenced schemas inlined + inline
            // objects) into one flat product type.
            let mut merged: HashMap<&'a str, IAST<'a>> = HashMap::new();
            let mut visited: HashSet<&'a str> = HashSet::new();
            collect_all_of_properties(ctx, all, &mut merged, &mut visited)?;
            if !merged.is_empty() {
                return Ok(IAST::Object(AnnotatedObj {
                    nullable: object.is_nullable().unwrap_or(false) || allows_null,
                    optional: is_optional,
                    is_deprecated: object.deprecated.unwrap_or(false),
                    description: object.description.as_deref(),
                    title: object.title.as_deref(),
                    value: AlgType::Product(merged),
                }));
            }
        }
    }

    println!("got to an empty object");

    //TODO: hmm
    Ok(IAST::Primitive(AnnotatedObj {
        // its value is already of type dynamic and therefor nullable internally
        nullable: false,
        optional: is_optional,
        is_deprecated: object.deprecated.unwrap_or(false),
        description: {
            let desc = &object.description;
            match desc {
                Some(desc) => Some(desc.as_str()),
                None => Some("Couldn't parse Object"),
            }
        },
        title: object.title.as_deref(),
        value: Primitive::Never,
    }))
}

/// Whether an object schema permits `null` — either via the OAS 3.0
/// `nullable: true` keyword or an OAS 3.1 type set that includes `"null"`
/// (e.g. `{"type": ["object", "null"]}`). Used to detect the nullable-`$ref`
/// `allOf` shape.
fn schema_allows_null(object: &ObjectSchema) -> bool {
    object.is_nullable().unwrap_or(false)
        || object
            .schema_type
            .as_ref()
            .map(|t| t.contains(SchemaType::Null))
            .unwrap_or(false)
}

/// Recursively collect the merged property set of an `allOf` list into `out`.
/// Inline object subschemas contribute their properties directly; `$ref`
/// subschemas have their target scheme's properties inlined. Nested `allOf`
/// (in either an inline object or a referenced object) is followed. Later
/// entries override earlier ones on key collision, matching JSON-Schema
/// `allOf` intersection semantics for this generator's flat product model.
/// `visited` guards against cyclic `$ref` chains.
fn collect_all_of_properties<'a>(
    ctx: &ParseCtx<'a>,
    schemas: &'a [ObjectOrReference<ObjectSchema>],
    out: &mut HashMap<&'a str, IAST<'a>>,
    visited: &mut HashSet<&'a str>,
) -> Result<(), Error> {
    for schema in schemas {
        match schema {
            ObjectOrReference::Object(obj) => {
                collect_object_properties(ctx, obj, out, visited)?;
            }
            ObjectOrReference::Ref { ref_path } => {
                let target = strip_ref_prefix(ref_path);
                if visited.insert(target) {
                    if let Some(ObjectOrReference::Object(obj)) = ctx.schemas.get(target) {
                        collect_object_properties(ctx, obj, out, visited)?;
                    }
                }
            }
        }
    }
    Ok(())
}

/// Merge a single object schema's own properties (and any nested `allOf`) into
/// `out`. Mirrors the property handling in `parse_object`'s `parse_properties`
/// closure, including the deprecated-field drop.
fn collect_object_properties<'a>(
    ctx: &ParseCtx<'a>,
    object: &'a ObjectSchema,
    out: &mut HashMap<&'a str, IAST<'a>>,
    visited: &mut HashSet<&'a str>,
) -> Result<(), Error> {
    if !object.all_of.is_empty() {
        collect_all_of_properties(ctx, &object.all_of, out, visited)?;
    }
    for (name, schema) in object.properties.iter() {
        let is_required = object.required.iter().any(|n| n.as_str() == name.as_str());
        let obj = parse_schema(ctx, schema, !is_required, false)?;
        if ctx.args.ignore_deprecated_fields && iast_is_deprecated(&obj) {
            continue;
        }
        out.insert(name.as_str(), obj);
    }
    Ok(())
}

fn convert_routes_to_tree<'a>(routes: &Vec<Route>) -> RouteFragment {
    let mut root_fragment = RouteFragmentNodeData {
        path_fragment_name: "".to_string(),
        is_param: false,
        children: vec![],
    };
    let mut branchless_route_trees = Vec::new();

    // first create a bunch of branchless trees
    for (idx, route) in routes.iter().enumerate() {
        let mut current_node = RouteFragment::Leaf(RouteFragmentLeafData { route_idx: idx });
        for segment in route.path.split('/').rev() {
            if segment.is_empty() {
                continue;
            }
            let is_param =
                segment.starts_with(':') || segment.starts_with('{') && segment.ends_with('}');
            let sanitized_segment = segment
                .trim_matches(':')
                .trim_matches('{')
                .trim_matches('}');

            let node = RouteFragment::Node(RouteFragmentNodeData {
                path_fragment_name: sanitized_segment.to_string(),
                is_param,
                children: vec![current_node],
            });
            current_node = node;
        }
        branchless_route_trees.push(current_node);
    }
    // now we need to merge the trees

    for tree in branchless_route_trees {
        merge_branchless_tree(&mut root_fragment, tree);
    }
    RouteFragment::Node(root_fragment)
}

fn merge_branchless_tree(main_tree: &mut RouteFragmentNodeData, branchless_tree: RouteFragment) {
    match branchless_tree {
        RouteFragment::Node(mut branchless_node) => {
            let mut matched_node = None;
            for child in main_tree.children.iter_mut() {
                match child {
                    RouteFragment::Node(ref mut node) => {
                        if node.path_fragment_name == branchless_node.path_fragment_name {
                            matched_node = Some(node);
                            break;
                        }
                    }
                    RouteFragment::Leaf(_) => {}
                }
            }
            match matched_node {
                Some(node) => {
                    merge_branchless_tree(node, branchless_node.children.pop().unwrap());
                }
                None => main_tree
                    .children
                    .push(RouteFragment::Node(branchless_node)),
            }
        }
        RouteFragment::Leaf(_) => main_tree.children.push(branchless_tree),
    }
}
