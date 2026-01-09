pub mod types;
use std::collections::{BTreeMap, HashMap};

use oas3::spec::Response as Responses;
use oas3::spec::*;
pub use types::*;
#[macro_use]
mod macros;

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    NoComponents,
    ParseError(String),
}

pub fn parse(spec: &oas3::Spec) -> Result<IntermediateFormat, Error> {
    let mut schemes = Vec::new();
    let components = match &spec.components {
        Some(components) => components,
        None => return Err(Error::NoComponents),
    };
    for (name, schema) in components.schemas.iter() {
        let obj = match parse_schema(schema, false, false) {
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
                            params_parser: Box::new(parse_params),
                            request_parser: Box::new(parse_request),
                            responses_parser: Box::new(parse_responses),
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

fn parse_params(params: &Vec<ObjectOrReference<Parameter>>) -> Result<Vec<Param>, Error> {
    params
        .iter()
        .map(|param| match param {
            ObjectOrReference::Object(Parameter {
                name,
                location,
                description,
                required,
                ..
            }) => {
                if location == &ParameterIn::Path {
                    None
                } else {
                    // println!("param: {}, required: {}", name, required.unwrap_or(false));
                    Some(Ok(Param {
                        name: name.as_str(),
                        description: description.as_deref(),
                        required: required.unwrap_or(false),
                    }))
                }
            }
            ObjectOrReference::Ref { ref_path, .. } => Some(Err(Error::ParseError(format!(
                "Reference to {} not supported in params",
                ref_path
            )))),
        })
        .filter_map(|param| param)
        .collect()
}

fn parse_request(request: Option<&ObjectOrReference<RequestBody>>) -> Result<IAST, Error> {
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
            parse_schema(scheme, false, false)
        }
        Some(ObjectOrReference::Ref { ref_path, .. }) => Ok(IAST::Reference(AnnotatedReference {
            path: ref_path,
            optional: false,
            nullable: false,
        })),
        None => Err(Error::ParseError("No request body".to_string())),
    }
}
fn parse_responses<'a>(
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
                    let schema = parse_schema(&schema, false, false).unwrap();
                    map.insert(code, schema);
                }
            }
            ObjectOrReference::Ref { ref_path, .. } => {
                let schema = IAST::Reference(AnnotatedReference {
                    path: ref_path,
                    optional: false,
                    nullable: false,
                });
                map.insert(code, schema);
            }
        }
    }
    Ok(map)
}

fn parse_schema(
    schema: &ObjectOrReference<ObjectSchema>,
    is_optional: bool,
    // only used in rare edge case where we have an object that is only a ref but can also be null
    ref_is_nullable: bool,
) -> Result<IAST, Error> {
    match schema {
        ObjectOrReference::Object(object) => parse_object(object, is_optional),
        ObjectOrReference::Ref { ref_path } => Ok(IAST::Reference(AnnotatedReference {
            path: ref_path,
            optional: is_optional,
            nullable: ref_is_nullable,
        })),
    }
}

fn parse_object(object: &ObjectSchema, is_optional: bool) -> Result<IAST, Error> {
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
                    .map(|(name, schema)| {
                        let is_required = object.required.iter().any(|n|n.as_str() == name.as_str());
                        // println!("parsing property: {}, nullable: {}", name, !is_required);

                        if name.starts_with("dummy") {
                            println!("v1001: {} optional:{:?} {:?}", name, !is_required, schema);
                        }

                        //TODO: there was a case where a required object could either be an object or null, but the oas3 spec properties where empty although the json had some, idk
                        match parse_schema(schema, !is_required, false) {
                            Ok(obj) => Ok((name.as_str(), obj)),
                            Err(e) => Err(e),
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
            Some(vec![(const_value.to_string(), const_value.is_string())])
        } else if !object.enum_values.is_empty() {
            Some(
                object
                    .enum_values
                    .iter()
                    .map(|v| (v.to_string().trim_matches('"').to_string(), v.is_string()))
                    .collect(),
            )
        } else {
            None
        };
        if let Some(mut enum_values) = enum_values {
            if let serde_json::Value::Bool(true) = object.extensions["x-extensible-enum"] {
                // our parser checks for unspecified, so add it
                enum_values.push(("unspecified".to_string(), false));
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
                Some(items) => match parse_schema(&items, false, false) {
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
        let descrimination_keys =match &object.discriminator {
            Some(Discriminator { mapping: Some(mapping), property_name }) => {
                Some(mapping.clone().into_keys().collect::<Vec<_>>())
            }
            _ => None,
        };
        let nullable = union_types.iter().any(|schema| match schema {
            ObjectOrReference::Object(schema) => 
                schema.is_nullable().unwrap_or(false) || 
                if let Some(SchemaTypeSet::Single(SchemaType::Null)) = schema.schema_type { true } else { false },
            ObjectOrReference::Ref { .. } => false,
        });
        return Ok(IAST::Object(AnnotatedObj {
            nullable,
            optional: is_optional,
            is_deprecated: object.deprecated.unwrap_or(false),
            description: object.description.as_deref(),
            title: object.title.as_deref(),
            value: AlgType::Sum(
                match union_types
                    .iter()
                    .enumerate()
                    .map(|(idx, schema)| 
                        match parse_schema(schema, false, nullable) {
                            Ok(obj) => Ok((match &descrimination_keys {
                                // https://linear.app/blingos/issue/DEV-4659/beam-make-union-type-typenames-more-friendly
                                //TODO: this might not be deterministic order...
                                // yeah, unfortunately it isnt..
                                // instead we actually have to check the values, but as most subtypes are behind a spec reference its hard to get to these values
                                // Some(keys) => keys[idx].clone(),
                                _ => idx.to_string(),
                            }, obj)),
                            Err(e) => {
                                return Err(e);
                            }
                        }
                    )
                    .collect::<Result<Vec<(_, _)>, _>>()
                {
                    Ok(types) => types,
                    Err(e) => return Err(e),
                },
            ),
        }));
    }

    // 2:
    if !object.properties.is_empty() {
        return parse_properties();
    }

    // 3:
    if !object.all_of.is_empty() {
        let all = &object.all_of;
        if all.len() == 2 && all.iter().any(|schema| matches!(schema, ObjectOrReference::Ref { .. })) {
            if let Some(ObjectOrReference::Ref { ref_path }) = all.iter().find(|schema| matches!(schema, ObjectOrReference::Ref { .. })) {
                return Ok(IAST::Reference(AnnotatedReference {
                    path: ref_path,
                    optional: is_optional,
                    nullable: true,
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
