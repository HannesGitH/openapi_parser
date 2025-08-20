use std::collections::HashMap;

use super::super::interface::*;

use crate::parse::intermediate::{AlgType, AnnotatedObj, Primitive};
#[allow(unused_imports)]
use crate::{cpf, parse::intermediate};

#[allow(non_upper_case_globals)]
static empty_str: String = String::new();

pub(super) struct SchemeAdder<'a> {
    class_prefix: &'a str,
    class_suffix: &'a str,
    vars_should_be_final: bool,
    complete_iast: Option<&'a intermediate::IntermediateFormat<'a>>,
}

impl<'a> SchemeAdder<'a> {
    pub(super) fn new(
        class_prefix: &'a str,
        class_suffix: &'a str,
        vars_should_be_final: bool,
    ) -> Self {
        Self {
            class_prefix,
            class_suffix,
            vars_should_be_final,
            complete_iast: None,
        }
    }

    pub(super) fn set_complete_iast(&mut self, intermediate: &'a intermediate::IntermediateFormat<'a>) {
        self.complete_iast = Some(intermediate);
    }

    pub(super) fn add_schemes(
        & self,
        out: &mut Vec<File>,
    ) {
        let mut scheme_files = Vec::new();
        for scheme in self.complete_iast.unwrap().schemes.iter() {
            let (mut content, depends_on_files, _, _nullable, _optional, _is_binary) =
                self.parse_named_iast(format!("{}{}", scheme.name, if scheme.is_inherently_nullable { "NonNull" } else { "" }).as_str(), &scheme.obj, 0);
            if scheme.is_inherently_nullable     {
                cpf!(content,"typedef {} = {}?;", self.class_name(scheme.name), self.class_name(format!("{}NonNull", scheme.name).as_str()));
            }
            let file = File {
                path: std::path::PathBuf::from(format!("{}.dart", scheme.name)),
                content,
            };
            scheme_files.push(file);
            scheme_files.extend(depends_on_files);
        }
        // add barrel file
        scheme_files.push(File {
            path: std::path::PathBuf::from("schemes.dart"),
            content: {
                let mut content = String::new();
                for file in scheme_files.iter() {
                    content.push_str(&format!("export '{}';\n", file.path.display()));
                }
                content
            },
        });
        // put all scheme files into the scheme directory
        out.extend(scheme_files.into_iter().map(|f| File {
            path: std::path::PathBuf::from(format!("schemes/{}", f.path.display())),
            content: f.content,
        }));
    }

    pub(super) fn class_name(&self, name: &str) -> String {
        format!("{}{}{}", self.class_prefix, name, self.class_suffix)
    }

    // return (content, files, not_built, nullable, optional)
    pub(super) fn parse_named_iast(
        &self,
        name: &str,
        iast: &intermediate::IAST,
        depth: usize,
    ) -> (String, Vec<File>, Option<GenerationSpecialCase>, bool, bool, bool) {
        match iast {
            intermediate::IAST::Object(annotated_obj) => {
                let doc_str = mk_doc_str(name, annotated_obj, 0);
                let alg_type = &annotated_obj.value;
                use intermediate::AlgType;
                match alg_type {
                    AlgType::Sum(sum) => {
                        let (content, files) = self.generate_sum_type(name, &doc_str, sum, depth);
                        let nullable = annotated_obj.nullable;
                        let optional = annotated_obj.optional;
                        (content, files, None, nullable, optional, false)
                    }
                    AlgType::Product(product) => {
                        let (content, files) =
                            self.generate_product_type(name, &doc_str, product, depth);
                        let nullable = annotated_obj.nullable;
                        let optional = annotated_obj.optional;
                        (content, files, None, nullable, optional, false)
                    }
                }
            }
            intermediate::IAST::Reference(annotated_ref) => {
                let link = annotated_ref.path;
                let trimmed_link = link.replace("#/components/schemas/", "");
                (
                    format!(
                        "export '{}schemes/{}.dart';\nimport '{}schemes/{}.dart';\n",
                        "../".repeat(depth + 1),
                        trimmed_link,
                        "../".repeat(depth + 1),
                        trimmed_link,
                    ),
                    vec![],
                    Some(GenerationSpecialCase {
                        type_name: self.class_name(&trimmed_link),
                        reason: GenerationSpecialCaseType::Link(trimmed_link),
                    }),
                    annotated_ref.nullable,
                    annotated_ref.optional,
                    // is binary: NO
                    false,
                )
            }
            intermediate::IAST::Primitive(annotated_obj) => {
                let doc_str = mk_doc_str(name, annotated_obj, 0);
                let mk_type_def = |name: &str, typ: &str, omit_import: bool| {
                    let mut ret = String::new();
                    if !omit_import {
                        ret.push_str(&format!(
                            "// ignore_for_file: unused_import\nimport '../{}utils/serde.dart';\n\n",
                            "../".repeat(depth)
                        ));
                        ret.push_str("import 'dart:typed_data';\n");
                    }
                    let name = self.class_name(name);
                    ret.push_str(&format!("{}typedef {} = {};\n", doc_str, name, typ));
                    ret
                };
                match &annotated_obj.value {
                    intermediate::types::Primitive::Enum(allowed_values) => {
                        let (class_name, content) = self.generate_primitive_sum_type(
                            name,
                            &doc_str,
                            &allowed_values
                                .iter()
                                .map(|v| (v.0.as_str(), v.1, empty_str.as_str()))
                                .collect::<Vec<(_, _, _)>>(),
                        );
                        let mut ret = String::new();
                        ret.push_str(&mk_type_def(name, &class_name, false));

                        ret.push_str(&content);
                        (ret, vec![], None, annotated_obj.nullable, annotated_obj.optional, false)
                    }
                    intermediate::types::Primitive::List(inner_iast) => {
                        let mut inner_name = &format!("{}_", name);
                        let (mut content, depends_on_files, inner_special_case, _nullable, _optional, _is_binary) =
                            self.parse_named_iast(&inner_name, inner_iast, depth);
                        let mut file_dependencies = Vec::new();

                        for f in depends_on_files.into_iter() {
                            file_dependencies.push(f);
                        }

                        if let Some(GenerationSpecialCase {
                            type_name: _,
                            reason: GenerationSpecialCaseType::Link(internal_type_name),
                        }) = &inner_special_case
                        {
                            println!("list of link: {}", internal_type_name);
                            inner_name = internal_type_name;
                        }

                        let outer_name =format!("List<{}>", self.class_name(&inner_name));

                        content.push_str(&mk_type_def(
                            name,
                            &outer_name,
                            true,
                        ));

                        (
                            content,
                            file_dependencies,
                            Some(GenerationSpecialCase {
                                reason: GenerationSpecialCaseType::List(
                                    self.class_name(&inner_name),
                                    matches!(
                                        inner_special_case,
                                        Some(GenerationSpecialCase {
                                            reason: GenerationSpecialCaseType::Primitive,
                                            ..
                                        })
                                    ),
                                ),
                                //XXX: use `self.class_name(name)` if we want the left part of the typedef 
                                // e.g. BEAM_v2_billing_subscriptions_subscribeMethods_postResponseModel
                                // or user `outer_name` for the right part
                                // e.g. List<BEAMSubscriptionModel>
                                type_name: outer_name,
                            }),
                            annotated_obj.nullable,
                            annotated_obj.optional,
                            false,
                        )
                    }
                    intermediate::types::Primitive::Binary => {
                        let typ = to_dart_prim(&annotated_obj.value);
                        (
                            mk_type_def(name, &typ, false),
                            vec![],
                            Some(GenerationSpecialCase {
                                reason: GenerationSpecialCaseType::Primitive,
                                type_name: typ,
                            }),
                            annotated_obj.nullable,
                            annotated_obj.optional,
                            true,
                        )
                    }
                    intermediate::types::Primitive::Never => {
                        let typ = to_dart_prim(&annotated_obj.value);
                        (
                            mk_type_def(name, &typ, false),
                            vec![],
                            None,
                            annotated_obj.nullable,
                            annotated_obj.optional,
                            false,
                        )
                    }
                    _ => {
                        let typ = to_dart_prim(&annotated_obj.value);
                        (
                            mk_type_def(name, &typ, false),
                            vec![],
                            Some(GenerationSpecialCase {
                                reason: GenerationSpecialCaseType::Primitive,
                                type_name: typ,
                            }),
                            annotated_obj.nullable,
                            annotated_obj.optional,
                            false,
                        )
                    }
                }
            }
        }
    }

    // return (content, files)
    fn generate_sum_type(
        &self,
        name: &str,
        doc_str: &str,
        // (name, type)
        sum: &Vec<(String, intermediate::IAST)>,
        depth: usize,
    ) -> (String, Vec<File>) {
        let class_name = self.class_name(name);
        let mut file_dependencies = Vec::new();
        let mut sub_file_dependencies = Vec::new();

        let index_to_name = |idx: &String| format!("{}{}", name, idx);

        let mut variants = Vec::new();

        for (union_inner_name, iast) in sum.iter() {
            let variant_name = index_to_name(union_inner_name);
            let (content, depends_on_files, not_built, nullable, optional, is_binary) =
                self.parse_named_iast(&variant_name, iast, depth + 1);
            file_dependencies.push(File {
                path: std::path::PathBuf::from(format!("{}/{}.dart", name, union_inner_name)),
                content,
            });
            variants.push((self.class_name(&variant_name), not_built, nullable));
            for f in depends_on_files.into_iter() {
                sub_file_dependencies.push(f);
            }
        }

        let mut content = String::new();

        content.push_str(&format!(
            "import '../{}utils/serde.dart';\n",
            "../".repeat(depth)
        ));

        for f in file_dependencies.iter() {
            content.push_str(&format!("import '{}';\n", f.path.display()));
            content.push_str(&format!("export '{}';\n", f.path.display()));
        }

        content.push_str(&format!(
            "\n{}sealed class {} implements BEAMSerde {{\n\t{}{}();",
            doc_str,
            class_name,
            if self.vars_should_be_final {
                "const "
            } else {
                ""
            },
            class_name
        ));
        content.push_str("\n\n\t@Deprecated(\"not deprecated, but usage is highly discouraged, as its not deterministic\")");
        content.push_str(&format!(
            "\n\tfactory {}.fromJson(dynamic json) {{",
            class_name
        ));
        for (v, _, variant_nullable) in variants.iter() {
            content.push_str(&format!(
                "\n\t\ttry{{\n\t\t\treturn {}_.fromJson(json);\n\t\t}} catch(e) {{}}",
                v
            ));
        }
        content.push_str(&format!(
            "\n\t\tthrow Exception('Could not parse json into {}');\n\t}}",
            class_name
        ));

        content.push_str("\n}\n\n");

        for (v, not_built, variant_nullable) in variants.iter() {
            let value_type_name = match not_built {
                Some(GenerationSpecialCase {
                    type_name: _,
                    reason: GenerationSpecialCaseType::Link(internal_type_name),
                }) => {
                    &self.class_name(internal_type_name)
                },
                _ => v,
            };

            content.push_str(&format!("class {}_ extends {} {{\n", v, class_name));
            content.push_str(&format!(
                "  {}{} value;\n",
                if self.vars_should_be_final {
                    "final "
                } else {
                    ""
                },
                value_type_name
            ));
            content.push_str(&format!(
                "  {}{}_(this.value);\n",
                if self.vars_should_be_final {
                    "const "
                } else {
                    ""
                },
                v
            ));
            content.push_str(&format!(
                "\n  @override\n  dynamic toJson() => {};\n",
                match not_built {
                    Some(GenerationSpecialCase {
                        reason: GenerationSpecialCaseType::Primitive,
                        type_name: _,
                    }) => "value".to_string(),
                    Some(GenerationSpecialCase {
                        reason: GenerationSpecialCaseType::List(inner_type, is_primitive),
                        type_name: _,
                    }) => if *is_primitive {
                        format!("value.map((e) => e as {}).toList()", inner_type)
                    } else {
                        format!("value.map((e) => e.toJson()).toList()")
                    },
                    _ => "value.toJson()".to_string(),
                }
            ));
            content.push_str(&format!(
                "  factory {}_.fromJson(dynamic json) => \n\t\t{}_({});\n",
                v,
                v,
                match not_built {
                    Some(GenerationSpecialCase {
                        reason: GenerationSpecialCaseType::Primitive,
                        type_name: _,
                    }) => "json".to_string(),
                    Some(GenerationSpecialCase {
                        reason: GenerationSpecialCaseType::List(inner_type, is_primitive),
                        type_name: _,
                    }) => if *is_primitive {
                        format!("(json as List).map((e) => e as {}).toList()", inner_type)
                    } else {
                        format!("(json as List).map((e) => {}.fromJson(e)).toList()", inner_type)
                    },
                    _ => format!("{}.fromJson(json)", value_type_name),
                }
            ));
            content.push_str("}\n\n");
        }

        file_dependencies.extend(sub_file_dependencies.into_iter().map(|f| File {
            path: std::path::PathBuf::from(format!("{}/{}", name, f.path.display())),
            content: f.content,
        }));
        (content, file_dependencies)
    }

    /// this is an enum
    /// return (enum_name, content)
    pub(super) fn generate_primitive_sum_type(
        &self,
        name: &str,
        doc_str: &str,
        // (allowed_value, is_string, description)
        allowed_values: &Vec<(&str, bool, &str)>,
    ) -> (String, String) {
        let class_name = format!("{}{}", self.class_prefix, name);
        // (sanitized_value, enum_value, is_string, description)
        let allowed_values_str = allowed_values
            .iter()
            .map(|v| (&v.0, sanitize(v.0), v.1, &v.2))
            .collect::<Vec<(_, _, _, _)>>();
        let mut content = String::new();
        content.push_str(&format!(
            "\n{}enum {} implements BEAMSerde {{\n",
            doc_str, class_name
        ));
        for (orig_value, enum_value, _, desc) in allowed_values_str.iter() {
            content.push_str(&format!("\n  /// {}\n", orig_value));
            content.push_str(&format!("  ///{}\n", desc.replace("\n", "\n  ///")));
            content.push_str(&format!("  t_{},\n", enum_value));
        }
        content.push_str(&"\t;\n\n\t@override\n\tdynamic toJson() => switch(this) {\n");
        for (orig_value, enum_value, is_string, _) in allowed_values_str.iter() {
            content.push_str(&format!("\t\tt_{} => {},\n", enum_value, if *is_string {
                format!("'{}'", orig_value)
            } else {
                orig_value.to_string()
            }));
        }
        content.push_str(&format!(
            "\t}};\n\tfactory {}.fromJson(dynamic json) => switch(json) {{\n",
            class_name
        ));
        for (orig_value, enum_value, is_string, _) in allowed_values_str.iter() {
            content.push_str(&format!("\t\t{} => t_{},\n", if *is_string {
                format!("'{}'", orig_value)
            } else {
                orig_value.to_string()
            }, enum_value));
        }
        content.push_str(&format!(
            "\t\t_ => throw UnreachableError('{}'),\n",
            class_name
        ));
        content.push_str("  };\n");
        content.push_str("}\n");
        (class_name, content)
    }

    fn generate_product_type(
        &self,
        name: &str,
        doc_str: &str,
        product: &HashMap<&str, intermediate::IAST>,
        depth: usize,
    ) -> (String, Vec<File>) {
        let class_name = self.class_name(name);
        let mut file_dependencies = Vec::new();
        let mut file_sub_dependencies = Vec::new();
        let mut properties: Vec<Property> = Vec::new();

        let mut extra_content = String::new();

        let sorted_props = {
            let mut props = product.iter().collect::<Vec<_>>();
            props.sort_by_key(|(p_name, _)| p_name.to_string());
            props
        };
        for (p_name, iast) in sorted_props.iter() {
            if let intermediate::IAST::Primitive(prim) = &iast {
                let (prim_type, prim_data) = match &prim.value {
                    intermediate::types::Primitive::Enum(allowed_values) => {
                        let full_name = format!("{}_{}", name, p_name);
                        let (class_name, content) = self.generate_primitive_sum_type(
                            &full_name,
                            doc_str,
                            &allowed_values
                                .iter()
                                .map(|v| (v.0.as_str(), v.1, empty_str.as_str()))
                                .collect::<Vec<(_, _, _)>>(),
                        );
                        extra_content.push_str(&content);
                        (class_name, PropertyType::Normal)
                    }
                    intermediate::types::Primitive::List(inner_iast) => {
                        let mut full_name = &format!("{}_{}", name, p_name);
                        let (content, depends_on_files, inner_special_case, _nullable, _optional, _is_binary) =
                            self.parse_named_iast(&full_name, inner_iast, depth + 1);

                        if let Some(GenerationSpecialCase {
                            type_name: _,
                            reason: GenerationSpecialCaseType::Link(internal_type_name),
                        }) = &inner_special_case
                        {
                            println!("list of link (product){} {}", internal_type_name, full_name);
                            full_name = internal_type_name;
                        }

                        file_dependencies.push(File {
                            path: std::path::PathBuf::from(format!("{}/{}.dart", name, p_name)),
                            content,
                        });
                        for f in depends_on_files.into_iter() {
                            file_sub_dependencies.push(f);
                        }
                        let inner_class_name = self.class_name(&full_name);
                        (
                            format!("{}<{}>", to_dart_prim(&prim.value), inner_class_name),
                            PropertyType::Primitive(PrimitivePropertyType::List {
                                inner_type: inner_class_name,
                                inner_is_primitive: match **inner_iast {
                                    intermediate::IAST::Primitive(_) => true,
                                    _ => false,
                                },
                            }),
                        )
                    }
                    intermediate::types::Primitive::Map(inner_iast) => {
                        let full_name = format!("{}_{}", name, p_name);
                        let (content, depends_on_files, _, _nullable, _optional, _is_binary) =
                            self.parse_named_iast(&full_name, inner_iast, depth + 1);
                        file_dependencies.push(File {
                            path: std::path::PathBuf::from(format!("{}/{}.dart", name, p_name)),
                            content,
                        });
                        for f in depends_on_files.into_iter() {
                            file_sub_dependencies.push(f);
                        }
                        (
                            format!(
                                "{}<String,{}>",
                                to_dart_prim(&prim.value),
                                self.class_name(&full_name)
                            ),
                            PropertyType::Primitive(PrimitivePropertyType::Default),
                        )
                    }
                    _ => (
                        to_dart_prim(&prim.value),
                        PropertyType::Primitive(PrimitivePropertyType::Default),
                    ),
                };
                properties.push(Property {
                    name: p_name,
                    typ: prim_type,
                    nullable: prim.nullable || prim.optional,
                    optional: prim.optional,
                    doc_str: mk_doc_str(p_name, &prim, 1),
                    prop_type: prim_data,
                });
                continue;
            }
            let full_name = format!("{}_{}", name, p_name);
            let mut type_name = self.class_name(&full_name);
            let (content, depends_on_files, special_case, nullable, optional, _is_binary) =
                self.parse_named_iast(&full_name, iast, depth + 1);
            if let Some(GenerationSpecialCase {
                reason: GenerationSpecialCaseType::Link(internal_type_name),
                type_name: _,
            }) = special_case
            {
                println!("link: {} {}", internal_type_name, type_name);
                //XXX: use `self.class_name(name)` if we want the left part of the typedef 
                // e.g. BEAM_v2_billing_subscriptions_subscribeMethods_postResponseModel
                // or user `outer_name` for the right part
                // e.g. List<BEAMSubscriptionModel>
                type_name = self.class_name(internal_type_name.as_str());
            }
            properties.push(Property {
                name: p_name,
                typ: type_name,
                nullable: nullable || optional,
                optional: optional,
                doc_str: "".to_string(),
                prop_type: PropertyType::Normal,
            });
            file_dependencies.push(File {
                path: std::path::PathBuf::from(format!("{}/{}.dart", name, p_name)),
                content,
            });
            for f in depends_on_files.into_iter() {
                file_sub_dependencies.push(f);
            }
        }

        let mut content = String::new();
        content.push_str(&format!(
            "import '../{}utils/serde.dart';\n",
            "../".repeat(depth)
        ));
        for f in file_dependencies.iter() {
            content.push_str(&format!("import '{}';\n", f.path.display()));
            content.push_str(&format!("export '{}';\n", f.path.display()));
        }
        content.push_str("\n\n");

        content.push_str(&format!(
            "{}class {} implements BEAMSerde {{\n",
            doc_str, class_name
        ));
        for prop in properties.iter() {
            content.push_str(&format!(
                "\n{}  {}{}{} {};\n",
                prop.doc_str,
                if self.vars_should_be_final {
                    "final "
                } else {
                    ""
                },
                prop.typ,
                if prop.nullable { "?" } else { "" },
                create_property_name(prop.name)
            ));
        }

        // constructor
        content.push_str(&format!(
            "\n\n  {}{}({{\n",
            if self.vars_should_be_final {
                "const "
            } else {
                ""
            },
            class_name
        ));
        for prop in properties.iter() {
            content.push_str(&format!(
                "    {}this.{},\n",
                if !prop.nullable { "required " } else { "" },
                create_property_name(prop.name)
            ));
        }
        content.push_str("  });\n");

        //to json
        content.push_str("\n\n  @override\n  Map<String,dynamic> toJson() => {\n");
        for prop in properties.iter() {
            content.push_str(&format!(
                "    {}'{}': {}{},\n",
                if prop.optional {
                    format!("if({} != null) ", create_property_name(prop.name))
                } else {
                    "".to_string()
                },
                prop.name,
                create_property_name(prop.name),
                if let PropertyType::Normal = prop.prop_type {
                    "?.toJson()"
                } else {
                    ""
                }
            ));
        }
        content.push_str("  };\n");

        //from json
        content.push_str(&format!(
            "\n  factory {}.fromJson(Map<String,dynamic> json) => {}(\n",
            class_name, class_name
        ));
        for prop in properties.iter() {
            content.push_str(&format!(
                "    {}: {},\n",
                create_property_name(prop.name),
                if let PropertyType::Primitive(prim) = &prop.prop_type {
                    match prim {
                        PrimitivePropertyType::List {
                            inner_type,
                            inner_is_primitive,
                        } => {
                            format!(
                                "{} (json['{}'] as List).map((e) => {}).toList()",
                                if prop.nullable {
                                    format!("json['{}'] == null ? null : ", prop.name)
                                } else {
                                    "".to_string()
                                },
                                prop.name,
                                if *inner_is_primitive {
                                    format!("e as {}", inner_type)
                                } else {
                                    format!("{}.fromJson(e)", inner_type)
                                },
                            )
                        }
                        PrimitivePropertyType::Default => {
                            format!("json['{}']", prop.name)
                        }
                    }
                } else {
                    format!(
                        "{}{}.fromJson(json['{}'])",
                        if prop.nullable {
                            format!("json['{}'] == null ? null : ", prop.name)
                        } else {
                            "".to_string()
                        },
                        prop.typ,
                        prop.name,
                    )
                }
            ));
        }
        content.push_str("  );\n");
        content.push_str("}\n");
        content.push_str(&extra_content);
        file_dependencies.extend(file_sub_dependencies.into_iter().map(|f| File {
            path: std::path::PathBuf::from(format!("{}/{}", name, f.path.display())),
            content: f.content,
        }));
        (content, file_dependencies)
    }
}

fn mk_doc_str<T>(name: &str, annotated_obj: &intermediate::AnnotatedObj<T>, tabs: usize) -> String {
    let mut doc_str = String::new();
    doc_str.push_str(&format!("{}/// {}\n", "\t".repeat(tabs), name));
    if let Some(title) = annotated_obj.title {
        doc_str.push_str(&format!("{}/// TITLE: {}\n", "\t".repeat(tabs), title));
    }
    if let Some(description) = annotated_obj.description {
        doc_str.push_str(&format!(
            "{}/// {}\n",
            "\t".repeat(tabs),
            description.replace("\n", format!("\n{}///", "\t".repeat(tabs)).as_str())
        ));
    }
    if annotated_obj.is_deprecated {
        doc_str.push_str(&format!("{}/// DEPRECATED\n", "\t".repeat(tabs)));
        doc_str.push_str(&format!("{}@deprecated\n", "\t".repeat(tabs)));
    }
    doc_str
}

pub fn sanitize(name: &str) -> String {
    let sanitized = name
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect::<String>();

    sanitized
}

pub fn create_property_name(name: &str) -> String {
    let sanitized = sanitize(name);
    if sanitized.starts_with('_') {
        format!("$private{}", sanitized)
    } else {
        sanitized
    }
}

fn to_dart_prim(primitive: &intermediate::types::Primitive) -> String {
    use intermediate::types::Primitive;
    match primitive {
        Primitive::String => "String".to_string(),
        Primitive::Number => "num".to_string(),
        Primitive::Integer => "int".to_string(),
        Primitive::Boolean => "bool".to_string(),
        Primitive::Never => "UnknownBEAMObject".to_string(),
        Primitive::List(_) => "List".to_string(),
        Primitive::Map(_) => "Map".to_string(),
        Primitive::Enum(_) => "Enum".to_string(),
        Primitive::Dynamic => "dynamic".to_string(),
        Primitive::Binary => "Uint8List".to_string(),
    }
}

struct Property<'a> {
    name: &'a str,
    typ: String,
    nullable: bool,
    optional: bool,
    doc_str: String,
    prop_type: PropertyType,
}

enum PropertyType {
    Normal,
    Primitive(PrimitivePropertyType),
}

enum PrimitivePropertyType {
    List {
        inner_type: String,
        inner_is_primitive: bool,
    },
    Default,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GenerationSpecialCase {
    pub reason: GenerationSpecialCaseType,
    pub type_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum GenerationSpecialCaseType {
    Primitive,
    //inner type
    Link(String),
    //inner type, is primitive
    List(String, bool),
}
