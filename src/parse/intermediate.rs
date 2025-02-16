pub mod types;
use std::collections::HashMap;

use oas3::spec::*;
pub use types::*;

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
        schemes.push(Scheme {
            name: name.as_str(),
            obj: match parse_schema(schema) {
                Ok(obj) => obj,
                Err(e) => return Err(e),
            },
        });
    }
    Ok(IntermediateFormat { schemes })
}

fn parse_schema(schema: &ObjectOrReference<ObjectSchema>) -> Result<IAST, Error> {
    match schema {
        ObjectOrReference::Object(object) => parse_object(object),
        ObjectOrReference::Ref { ref_path, .. } => Ok(IAST::Reference(ref_path)),
    }
}

fn parse_object(object: &ObjectSchema) -> Result<IAST, Error> {
    let parse_properties = || {
        Ok(IAST::Object(AnnotatedObj {
            nullable: false,
            is_deprecated: object.deprecated.unwrap_or(false),
            description: object.description.as_deref(),
            title: object.title.as_deref(),
            value: AlgType::Product(
                match object
                    .properties
                    .iter()
                    .map(|(name, schema)| match parse_schema(schema) {
                        Ok(obj) => Ok((name.as_str(), obj)),
                        Err(e) => Err(e),
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
        let enum_values = if let Some(const_value) = &object.const_value {
            Some(vec![const_value.to_string()])
        } else if !object.enum_values.is_empty() {
            Some(object.enum_values.iter().map(|v| v.to_string()).collect())
        } else {
            None
        };
        if let Some(enum_values) = enum_values {
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
            SchemaType::Array => {
                match &object.items {
                    Some(items) => {
                        match parse_schema(&items) {
                            Ok(obj) => Primitive::List(Box::new(obj)),
                            Err(e) => {
                                println!("error parsing list: {:?}", e);
                                Primitive::Dynamic
                            }
                        }
                    }
                    None => {
                        println!("error parsing list, no items");
                        Primitive::Dynamic
                    }
            }},
        }
    };
    // if type is set, we can return a primitive type
    if let Some(types) = &object.schema_type {
        let prim_type = match types {
            SchemaTypeSet::Single(typ) => {
                println!("single type: {:?}", typ);
                typ
            }
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

    // 1:
    if !object.any_of.is_empty() || !object.one_of.is_empty() {
        // it cannot have both so we just precede with any_of
        let union_types = if !object.any_of.is_empty() {
            &object.any_of
        } else {
            &object.one_of
        };
        return Ok(IAST::Object(AnnotatedObj {
            //TODO: maybe check if null is in the union types
            nullable: false,
            is_deprecated: object.deprecated.unwrap_or(false),
            description: object.description.as_deref(),
            title: object.title.as_deref(),
            value: AlgType::Sum(
                match union_types
                    .iter()
                    .map(parse_schema)
                    .collect::<Result<Vec<_>, _>>()
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

    //TODO: hmm
    Ok(IAST::Primitive(AnnotatedObj {
        nullable: true,
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

    // AnnotatedObj {
    //     nullable: object.nullable,
    //     is_deprecated: object.deprecated.unwrap_or(false),
    //     description: object.description.as_deref(),
    //     title: object.title.as_deref(),
    //     value: object.properties,
    // }
}
