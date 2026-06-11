use std::collections::HashMap;

use super::super::interface::*;

use crate::parse::intermediate::{strip_ref_prefix, AnnotatedObj, Primitive};
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

    pub(super) fn set_complete_iast(
        &mut self,
        intermediate: &'a intermediate::IntermediateFormat<'a>,
    ) {
        self.complete_iast = Some(intermediate);
    }

    /// Thin wrapper that asks the shared
    /// [`crate::parse::intermediate::IntermediateFormat::resolve_iast`]
    /// classifier whether the given IAST resolves (after transitively
    /// following `Reference`s) to a raw Dart primitive without
    /// `.toJson()` / `.fromJson()`.
    ///
    /// `Enum` primitives are intentionally NOT counted as primitive
    /// because the generator emits a real Dart enum class with
    /// `.fromJson` for them.
    fn iast_resolves_to_primitive(&self, iast: &intermediate::IAST<'a>) -> bool {
        match self.complete_iast {
            Some(iformat) => iformat.resolve_iast(iast).is_primitive(),
            None => false,
        }
    }

    pub(super) fn add_schemes(&self, out: &mut Vec<File>) {
        let mut scheme_files = Vec::new();
        for scheme in self.complete_iast.unwrap().schemes.iter() {
            let sanitized_scheme_name = sanitize(scheme.name);
            let mut parsed = self.parse_named_iast(
                format!(
                    "{}{}",
                    sanitized_scheme_name,
                    if scheme.is_inherently_nullable {
                        "NonNull"
                    } else {
                        ""
                    }
                )
                .as_str(),
                &scheme.obj,
                0,
            );

            if scheme.is_inherently_nullable {
                // lol irgendwann sollte man mal auf ne templating engine umsteigen
                cpf!(
                    parsed.content,
                    "
class BEAM{}Model implements BEAMSerde {{

    BEAM{}Model(this.value);
    final BEAM{}NonNullModel? value;
    factory BEAM{}Model.fromJson(Map<String, dynamic>? json) {{
        if (json == null) {{
            return BEAM{}Model(null);
        }}
        return BEAM{}Model(BEAM{}NonNullModel.fromJson(json));
    }}

    toJson() => value?.toJson();
}}
                        ",
                    sanitized_scheme_name,
                    sanitized_scheme_name,
                    sanitized_scheme_name,
                    sanitized_scheme_name,
                    sanitized_scheme_name,
                    sanitized_scheme_name,
                    sanitized_scheme_name,
                );
            }
            let file = File {
                path: std::path::PathBuf::from(format!("{}.dart", sanitized_scheme_name)),
                content: parsed.content,
            };
            scheme_files.push(file);
            scheme_files.extend(parsed.files);
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
        format!(
            "{}{}{}",
            self.class_prefix,
            sanitize(name),
            self.class_suffix
        )
    }

    pub(super) fn parse_named_iast(
        &self,
        name: &str,
        iast: &intermediate::IAST,
        depth: usize,
    ) -> ParsedIast {
        match iast {
            intermediate::IAST::Object(annotated_obj) => {
                let doc_str = mk_doc_str(name, annotated_obj, 0);
                let alg_type = &annotated_obj.value;
                use intermediate::AlgType;
                match alg_type {
                    AlgType::Sum(sum) => {
                        let generated = self.generate_sum_type(name, &doc_str, sum, depth);
                        ParsedIast {
                            content: generated.content,
                            files: generated.files,
                            special_case: None,
                            nullable: annotated_obj.nullable,
                            optional: annotated_obj.optional,
                            is_binary: false,
                        }
                    }
                    AlgType::DiscriminatedSum(discrimination) => {
                        let generated = self.generate_discriminated_sum_type(
                            name,
                            &doc_str,
                            discrimination,
                            depth,
                        );
                        ParsedIast {
                            content: generated.content,
                            files: generated.files,
                            special_case: None,
                            nullable: annotated_obj.nullable,
                            optional: annotated_obj.optional,
                            is_binary: false,
                        }
                    }
                    AlgType::Product(product) => {
                        let generated = self.generate_product_type(name, &doc_str, product, depth);
                        ParsedIast {
                            content: generated.content,
                            files: generated.files,
                            special_case: None,
                            nullable: annotated_obj.nullable,
                            optional: annotated_obj.optional,
                            is_binary: false,
                        }
                    }
                }
            }
            intermediate::IAST::Reference(annotated_ref) => {
                let link = annotated_ref.path;
                let trimmed_link = sanitize(strip_ref_prefix(link));
                ParsedIast {
                    // some references are nullable also (this should not be, but leons vibes introduce them nontheless), so we need to add the serde import anyway
                    content: format!(
                        "// ignore_for_file: unused_import\nimport '{}utils/serde.dart';\nexport '{}schemes/{}.dart';\nimport '{}schemes/{}.dart';\n",
                        "../".repeat(depth + 1),
                        "../".repeat(depth + 1),
                        trimmed_link,
                        "../".repeat(depth + 1),
                        trimmed_link,
                    ),
                    files: vec![],
                    special_case: Some(GenerationSpecialCase {
                        type_name: self.class_name(&trimmed_link),
                        reason: GenerationSpecialCaseType::Link(trimmed_link),
                    }),
                    nullable: annotated_ref.nullable,
                    optional: annotated_ref.optional,
                    // is binary: NO
                    is_binary: false,
                }
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
                        let enum_code = self.generate_primitive_sum_type(
                            name,
                            &doc_str,
                            &allowed_values
                                .iter()
                                .map(|v| AllowedValue {
                                    value: v.value.as_str(),
                                    is_string: v.is_string,
                                    description: empty_str.as_str(),
                                })
                                .collect::<Vec<_>>(),
                        );
                        let mut ret = String::new();
                        ret.push_str(&mk_type_def(name, &enum_code.class_name, false));

                        ret.push_str(&enum_code.content);
                        ParsedIast {
                            content: ret,
                            files: vec![],
                            // enums are not considered primitive because e.g. they need to be parsed with .fromJson
                            special_case: None,
                            nullable: annotated_obj.nullable,
                            optional: annotated_obj.optional,
                            is_binary: false,
                        }
                    }
                    intermediate::types::Primitive::List(inner_iast) => {
                        let mut inner_name = &format!("{}_", name);
                        let mut inner = self.parse_named_iast(&inner_name, inner_iast, depth);
                        let mut file_dependencies = Vec::new();

                        for f in inner.files.into_iter() {
                            file_dependencies.push(f);
                        }

                        if let Some(GenerationSpecialCase {
                            type_name: _,
                            reason: GenerationSpecialCaseType::Link(internal_type_name),
                        }) = &inner.special_case
                        {
                            println!("list of link: {}", internal_type_name);
                            inner_name = internal_type_name;
                        }

                        let outer_name = format!("List<{}>", self.class_name(&inner_name));

                        inner
                            .content
                            .push_str(&mk_type_def(name, &outer_name, true));

                        ParsedIast {
                            content: inner.content,
                            files: file_dependencies,
                            special_case: Some(GenerationSpecialCase {
                                reason: GenerationSpecialCaseType::List(
                                    // the type of elements in the list
                                    self.class_name(&inner_name),
                                    // Whether the elements in the list are
                                    // a primitive Dart value (no
                                    // `.toJson` / `.fromJson`). True for
                                    // inline primitives, AND for `$ref`s
                                    // whose target scheme resolves to a
                                    // primitive typedef (`typedef Foo =
                                    // String;` etc). Enums are NOT
                                    // primitive here because the generator
                                    // emits a real Dart enum class with
                                    // `.fromJson` for them. The chain-
                                    // following is handled by the shared
                                    // resolver, so multi-hop refs
                                    // (`Alias -> Id -> string`) behave the
                                    // same as a direct ref.
                                    self.iast_resolves_to_primitive(inner_iast),
                                ),
                                //XXX: use `self.class_name(name)` if we want the left part of the typedef
                                // e.g. BEAM_v2_billing_subscriptions_subscribeMethods_postResponseModel
                                // or user `outer_name` for the right part
                                // e.g. List<BEAMSubscriptionModel>
                                type_name: outer_name,
                            }),
                            nullable: annotated_obj.nullable,
                            optional: annotated_obj.optional,
                            is_binary: false,
                        }
                    }
                    intermediate::types::Primitive::Binary => {
                        let typ = to_dart_prim(&annotated_obj.value);
                        ParsedIast {
                            content: mk_type_def(name, &typ, false),
                            files: vec![],
                            special_case: Some(GenerationSpecialCase {
                                reason: GenerationSpecialCaseType::Primitive,
                                type_name: typ,
                            }),
                            nullable: annotated_obj.nullable,
                            optional: annotated_obj.optional,
                            is_binary: true,
                        }
                    }
                    intermediate::types::Primitive::Never => {
                        let typ = to_dart_prim(&annotated_obj.value);
                        ParsedIast {
                            content: mk_type_def(name, &typ, false),
                            files: vec![],
                            special_case: None,
                            // this is never nullable, as its never (simplifies usage a bit, by not having to set an explicit value)
                            nullable: false,
                            optional: annotated_obj.optional,
                            is_binary: false,
                        }
                    }
                    _ => {
                        let typ = to_dart_prim(&annotated_obj.value);
                        ParsedIast {
                            content: mk_type_def(name, &typ, false),
                            files: vec![],
                            special_case: Some(GenerationSpecialCase {
                                reason: GenerationSpecialCaseType::Primitive,
                                type_name: typ,
                            }),
                            nullable: annotated_obj.nullable,
                            optional: annotated_obj.optional,
                            is_binary: false,
                        }
                    }
                }
            }
        }
    }

    fn generate_sum_type(
        &self,
        name: &str,
        doc_str: &str,
        sum: &[intermediate::types::SumVariant],
        depth: usize,
    ) -> GeneratedCode {
        let class_name = self.class_name(name);
        let mut file_dependencies = Vec::new();
        let mut sub_file_dependencies = Vec::new();

        let index_to_name = |idx: &String| format!("{}{}", name, idx);

        let mut variants = Vec::new();

        for intermediate::types::SumVariant {
            name: union_inner_name,
            typ: iast,
        } in sum.iter()
        {
            let sanitized_inner_name = sanitize(union_inner_name);
            let mut variant_name = index_to_name(&sanitized_inner_name);
            let parsed = self.parse_named_iast(&variant_name, iast, depth + 1);
            if let Some(GenerationSpecialCase {
                type_name: _,
                reason: GenerationSpecialCaseType::Link(internal_type_name),
            }) = &parsed.special_case
            {
                variant_name = index_to_name(&internal_type_name);
            }
            file_dependencies.push(File {
                path: std::path::PathBuf::from(format!("{}/{}.dart", name, sanitized_inner_name)),
                content: parsed.content,
            });
            variants.push(SumVariantClass {
                class_name: self.class_name(&variant_name),
                special_case: parsed.special_case,
            });
            for f in parsed.files.into_iter() {
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
            "\n\tfactory {}.fromJson(dynamic json) {{\n\t\tfinal errors = <String,Object>{{}};",
            class_name
        ));
        for variant in variants.iter() {
            content.push_str(&format!(
                "\n\t\ttry{{\n\t\t\treturn {}_.fromJson(json);\n\t\t}} catch(e) {{errors['{}']=e;}}",
                variant.class_name, variant.class_name
            ));
        }
        content.push_str(&format!(
            "\n\t\tthrow BEAMUnionParseMultiError(errors);\n\t}}",
        ));

        content.push_str("\n}\n\n");

        for variant in variants.iter() {
            let not_built = &variant.special_case;
            let value_type_name = match not_built {
                Some(GenerationSpecialCase {
                    type_name: _,
                    reason: GenerationSpecialCaseType::Link(internal_type_name),
                }) => &self.class_name(internal_type_name),
                _ => &variant.class_name,
            };

            content.push_str(&format!(
                "class {}_ extends {} {{\n",
                variant.class_name, class_name
            ));
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
                variant.class_name
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
                    }) =>
                        if *is_primitive {
                            format!("value.map((e) => e as {}).toList()", inner_type)
                        } else {
                            format!("value.map((e) => e.toJson()).toList()")
                        },
                    _ => "value.toJson()".to_string(),
                }
            ));
            content.push_str(&format!(
                "  factory {}_.fromJson(dynamic json) => \n\t\t{}_({});\n",
                variant.class_name,
                variant.class_name,
                match not_built {
                    Some(GenerationSpecialCase {
                        reason: GenerationSpecialCaseType::Primitive,
                        type_name: _,
                    }) => "json".to_string(),
                    Some(GenerationSpecialCase {
                        reason: GenerationSpecialCaseType::List(inner_type, is_primitive),
                        type_name: _,
                    }) =>
                        if *is_primitive {
                            format!("(json as List).map((e) => e as {}).toList()", inner_type)
                        } else {
                            format!(
                                "(json as List).map((e) => {}.fromJson(e)).toList()",
                                inner_type
                            )
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
        GeneratedCode {
            content,
            files: file_dependencies,
        }
    }

    /// Generates the "super" response union for an endpoint that has more
    /// than one status code.
    ///
    /// Unlike [`Self::generate_sum_type`], each arm is keyed by its HTTP
    /// status `code` rather than by the resolved schema name, so two codes
    /// that point at the *same* schema (e.g. `400`, `401`, `403` all using
    /// one error model) produce distinct arms instead of colliding into a
    /// single duplicated class.
    ///
    /// In addition to the (discouraged, non-deterministic) try-each
    /// `fromJson`, it exposes `fromCode(int statusCode, json)` which decodes
    /// the arm matching the status code, or returns `null` for an unknown
    /// code. The per-code response schemas themselves are generated by the
    /// caller and only referenced here via `import_path`.
    ///
    /// `depth` is the same value passed to [`Self::parse_named_iast`] for the
    /// per-code responses, so the `utils/serde.dart` import resolves
    /// identically to the other generated schema files in the directory.
    pub(super) fn generate_response_union(
        &self,
        super_name: &str,
        variants: &[ResponseUnionVariant],
        depth: usize,
    ) -> String {
        let class_name = self.class_name(super_name);
        let final_kw = if self.vars_should_be_final {
            "final "
        } else {
            ""
        };
        let const_kw = if self.vars_should_be_final {
            "const "
        } else {
            ""
        };
        // Per-code wrapper class name. Keying on the status code keeps arms
        // unique even when two codes resolve to the same value type.
        let variant_class =
            |v: &ResponseUnionVariant| self.class_name(&format!("{}_{}", super_name, v.code));

        let mut content = String::new();
        content.push_str(&format!(
            "import '../{}utils/serde.dart';\n",
            "../".repeat(depth)
        ));
        for v in variants {
            content.push_str(&format!("import '{}';\n", v.import_path));
            content.push_str(&format!("export '{}';\n", v.import_path));
        }

        // The union implements `BeamStatusCodeResponse` (itself a
        // `BEAMSerde`) so a handler can recognise status-code-keyed responses
        // via `is BeamStatusCodeResponse` and decode them through `fromCode`.
        content.push_str(&format!(
            "\n/// {}\nsealed class {} implements BeamStatusCodeResponse {{\n\t{}{}();",
            super_name, class_name, const_kw, class_name
        ));

        // Try-each `fromJson`: order-dependent and ambiguous when arms share
        // a shape, hence discouraged in favour of `fromCode`.
        content.push_str("\n\n\t@Deprecated(\"not deprecated, but usage is highly discouraged, as its not deterministic\")");
        content.push_str(&format!(
            "\n\tfactory {}.fromJson(dynamic json) {{\n\t\tfinal errors = <String,Object>{{}};",
            class_name
        ));
        for v in variants {
            let vc = variant_class(v);
            content.push_str(&format!(
                "\n\t\ttry{{\n\t\t\treturn {}_.fromJson(json);\n\t\t}} catch(e) {{errors['{}']=e;}}",
                vc, vc
            ));
        }
        content.push_str("\n\t\tthrow BEAMUnionParseMultiError(errors);\n\t}");

        // Deterministic status-code dispatch, as a switch expression.
        content.push_str(&format!(
            "\n\n\tstatic {}? fromCode(int statusCode, dynamic json) {{\n\t\treturn switch (statusCode) {{",
            class_name
        ));
        // A `default` response (if any) becomes the wildcard arm; otherwise
        // an unknown code yields null.
        let mut default_arm = "null".to_string();
        for v in variants {
            let vc = variant_class(v);
            if v.code == "default" {
                default_arm = format!("{}_.fromJson(json)", vc);
            } else if let Ok(code_num) = v.code.parse::<u16>() {
                content.push_str(&format!("\n\t\t\t{} => {}_.fromJson(json),", code_num, vc));
            }
            // Non-numeric, non-"default" codes (e.g. ranges like "2XX")
            // can't be matched by an int switch; they remain reachable only
            // through `fromJson`.
        }
        content.push_str(&format!("\n\t\t\t_ => {},\n\t\t}};\n\t}}", default_arm));

        content.push_str("\n}\n\n");

        for v in variants {
            let vc = variant_class(v);
            let to_json = match (&v.list_inner_type, v.is_primitive) {
                (Some(inner), true) => format!("value.map((e) => e as {}).toList()", inner),
                (Some(_), false) => "value.map((e) => e.toJson()).toList()".to_string(),
                (None, true) => "value".to_string(),
                (None, false) => "value.toJson()".to_string(),
            };
            let from_json = match (&v.list_inner_type, v.is_primitive) {
                (Some(inner), true) => {
                    format!("(json as List).map((e) => e as {}).toList()", inner)
                }
                (Some(inner), false) => {
                    format!("(json as List).map((e) => {}.fromJson(e)).toList()", inner)
                }
                (None, true) => "json".to_string(),
                (None, false) => format!("{}.fromJson(json)", v.value_type),
            };
            content.push_str(&format!("class {}_ extends {} {{\n", vc, class_name));
            content.push_str(&format!("  {}{} value;\n", final_kw, v.value_type));
            content.push_str(&format!("  {}{}_(this.value);\n", const_kw, vc));
            content.push_str(&format!(
                "\n  @override\n  dynamic toJson() => {};\n",
                to_json
            ));
            content.push_str(&format!(
                "  factory {}_.fromJson(dynamic json) => \n\t\t{}_({});\n",
                vc, vc, from_json
            ));
            content.push_str("}\n\n");
        }

        content
    }

    /// Generates a discriminated sum type (tagged union).
    /// Similar to `generate_sum_type` but simpler - parses the discriminator key first
    /// and switches on it instead of using try-catch.
    fn generate_discriminated_sum_type(
        &self,
        name: &str,
        doc_str: &str,
        discrimination: &intermediate::types::Discrimination,
        depth: usize,
    ) -> GeneratedCode {
        let class_name = self.class_name(name);
        let file_dependencies = Vec::new();

        // Build variants from discrimination map
        let mut variants: Vec<DiscriminatedVariant> = Vec::new();

        for (discriminator_value, annotated_ref) in discrimination.map.iter() {
            let trimmed_link = sanitize(strip_ref_prefix(annotated_ref.path));
            variants.push(DiscriminatedVariant {
                class_name: self.class_name(&format!("{}{}", name, trimmed_link)),
                discriminator_value,
                referenced_type_name: self.class_name(&trimmed_link),
            });
        }

        let mut content = String::new();

        content.push_str(&format!(
            "import '../{}utils/serde.dart';\n",
            "../".repeat(depth)
        ));

        // Import all referenced types
        for variant in variants.iter() {
            let referenced_type_name = &variant.referenced_type_name;
            let trimmed = referenced_type_name
                .strip_prefix(self.class_prefix)
                .unwrap_or(referenced_type_name)
                .strip_suffix(self.class_suffix)
                .unwrap_or(referenced_type_name);
            content.push_str(&format!(
                "import '../{}schemes/{}.dart';\n",
                "../".repeat(depth),
                trimmed
            ));
        }

        // Sealed class definition
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

        // fromJson factory with switch on discriminator
        content.push_str(&format!(
            "\n\n\tfactory {}.fromJson(Map<String, dynamic> json) {{\n",
            class_name
        ));
        content.push_str(&format!(
            "\t\tfinal discriminator = json['{}'];\n",
            discrimination.key
        ));
        content.push_str("\t\treturn switch(discriminator) {\n");

        for variant in variants.iter() {
            content.push_str(&format!(
                "\t\t\t'{}' => {}_({}.fromJson(json)),\n",
                variant.discriminator_value, variant.class_name, variant.referenced_type_name
            ));
        }

        content.push_str(&format!(
            "\t\t\t_ => throw BEAMUnknownValueError('{}: unknown discriminator value $discriminator'),\n",
            class_name
        ));
        content.push_str("\t\t};\n");
        content.push_str("\t}\n");

        content.push_str("}\n\n");

        // Generate variant classes
        for variant in variants.iter() {
            content.push_str(&format!(
                "class {}_ extends {} {{\n",
                variant.class_name, class_name
            ));
            content.push_str(&format!(
                "  {}{} value;\n",
                if self.vars_should_be_final {
                    "final "
                } else {
                    ""
                },
                variant.referenced_type_name
            ));
            content.push_str(&format!(
                "  {}{}_({} this.value);\n",
                if self.vars_should_be_final {
                    "const "
                } else {
                    ""
                },
                variant.class_name,
                variant.referenced_type_name
            ));
            content.push_str("\n  @override\n  dynamic toJson() => value.toJson();\n");
            content.push_str("}\n\n");
        }

        GeneratedCode {
            content,
            files: file_dependencies,
        }
    }

    /// this is an enum
    /// return (enum_name, content)
    pub(super) fn generate_primitive_sum_type(
        &self,
        name: &str,
        doc_str: &str,
        allowed_values: &[AllowedValue],
    ) -> EnumCode {
        let class_name = format!("{}{}", self.class_prefix, sanitize(name));
        // (sanitized_value, enum_value, is_string, description)
        let allowed_values_str = allowed_values
            .iter()
            .map(|v| (&v.value, sanitize(v.value), v.is_string, &v.description))
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
            content.push_str(&format!(
                "\t\tt_{} => {},\n",
                enum_value,
                if *is_string {
                    format!("'{}'", orig_value)
                } else {
                    orig_value.to_string()
                }
            ));
        }
        content.push_str(&format!(
            "\t}};\n\tfactory {}.fromJson(dynamic json) => switch(json) {{\n",
            class_name
        ));
        let allows_unspecified = allowed_values_str
            .iter()
            .any(|(_, enum_value, _, _)| enum_value == "unspecified");
        for (orig_value, enum_value, is_string, _) in allowed_values_str.iter() {
            content.push_str(&format!(
                "\t\t{} => t_{},\n",
                if *is_string {
                    format!("'{}'", orig_value)
                } else {
                    orig_value.to_string()
                },
                enum_value
            ));
        }
        content.push_str(&format!(
            "\t\tdynamic s => {}",
            if allows_unspecified {
                "t_unspecified".to_string()
            } else {
                format!(
                    "throw BEAMUnknownValueError('{}: unknown value $s'),\n",
                    class_name
                )
            }
        ));
        content.push_str("  };\n");
        content.push_str("}\n");
        EnumCode {
            class_name,
            content,
        }
    }

    fn generate_product_type(
        &self,
        name: &str,
        doc_str: &str,
        product: &HashMap<&str, intermediate::IAST>,
        depth: usize,
    ) -> GeneratedCode {
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
            let sanitized_p_name = sanitize(p_name);
            if let intermediate::IAST::Primitive(prim) = &iast {
                let (prim_type, prim_data) = match &prim.value {
                    intermediate::types::Primitive::Enum(allowed_values) => {
                        let full_name = format!("{}_{}", name, sanitized_p_name);
                        let enum_code = self.generate_primitive_sum_type(
                            &full_name,
                            doc_str,
                            &allowed_values
                                .iter()
                                .map(|v| AllowedValue {
                                    value: v.value.as_str(),
                                    is_string: v.is_string,
                                    description: empty_str.as_str(),
                                })
                                .collect::<Vec<_>>(),
                        );
                        extra_content.push_str(&enum_code.content);
                        (enum_code.class_name, PropertyType::Normal)
                    }
                    intermediate::types::Primitive::List(inner_iast) => {
                        let mut full_name = &format!("{}_{}", name, sanitized_p_name);
                        let parsed = self.parse_named_iast(&full_name, inner_iast, depth + 1);

                        if let Some(GenerationSpecialCase {
                            type_name: _,
                            reason: GenerationSpecialCaseType::Link(internal_type_name),
                        }) = &parsed.special_case
                        {
                            println!("list of link (product){} {}", internal_type_name, full_name);
                            full_name = internal_type_name;
                        }

                        file_dependencies.push(File {
                            path: std::path::PathBuf::from(format!(
                                "{}/{}.dart",
                                name, sanitized_p_name
                            )),
                            content: parsed.content,
                        });
                        for f in parsed.files.into_iter() {
                            file_sub_dependencies.push(f);
                        }
                        let inner_class_name = self.class_name(&full_name);
                        (
                            format!("{}<{}>", to_dart_prim(&prim.value), inner_class_name),
                            PropertyType::Primitive(PrimitivePropertyType::List {
                                inner_type: inner_class_name,
                                inner_is_primitive: match &**inner_iast {
                                    // enums are not considered primitive in dart because e.g. they need to be parsed with .fromJson
                                    intermediate::IAST::Primitive(AnnotatedObj {
                                        value: Primitive::Enum(_),
                                        ..
                                    }) => false,
                                    intermediate::IAST::Primitive(_) => true,
                                    // A reference may point at a scheme that is
                                    // itself just a primitive typedef (e.g.
                                    // `typedef BEAMCmsObjectIdParamModel = String;`).
                                    // In that case the list elements have no
                                    // `.toJson()` / `.fromJson()` and must be
                                    // treated as primitives.
                                    intermediate::IAST::Reference(_) => {
                                        self.iast_resolves_to_primitive(inner_iast)
                                    }
                                    _ => false,
                                },
                            }),
                        )
                    }
                    intermediate::types::Primitive::Map(inner_iast) => {
                        let full_name = format!("{}_{}", name, sanitized_p_name);
                        let parsed = self.parse_named_iast(&full_name, inner_iast, depth + 1);
                        file_dependencies.push(File {
                            path: std::path::PathBuf::from(format!(
                                "{}/{}.dart",
                                name, sanitized_p_name
                            )),
                            content: parsed.content,
                        });
                        for f in parsed.files.into_iter() {
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
            let full_name = format!("{}_{}", name, sanitized_p_name);
            let mut type_name = self.class_name(&full_name);
            let parsed = self.parse_named_iast(&full_name, iast, depth + 1);
            if let Some(GenerationSpecialCase {
                reason: GenerationSpecialCaseType::Link(internal_type_name),
                type_name: _,
            }) = parsed.special_case
            {
                println!("link: {} {}", internal_type_name, type_name);
                //XXX: use `self.class_name(name)` if we want the left part of the typedef
                // e.g. BEAM_v2_billing_subscriptions_subscribeMethods_postResponseModel
                // or user `outer_name` for the right part
                // e.g. List<BEAMSubscriptionModel>
                type_name = self.class_name(internal_type_name.as_str());
            }
            // A reference whose target scheme is a primitive typedef
            // (e.g. `typedef BEAMCmsObjectIdParamModel = String;`) must not
            // have `.toJson()` / `.fromJson()` called on it, because Dart
            // built-in types like `String` don't expose those methods.
            // Detect that case and fall through to the primitive code path.
            let prop_type = if matches!(iast, intermediate::IAST::Reference(_))
                && self.iast_resolves_to_primitive(iast)
            {
                PropertyType::Primitive(PrimitivePropertyType::Default)
            } else {
                PropertyType::Normal
            };
            properties.push(Property {
                name: p_name,
                typ: type_name,
                nullable: parsed.nullable || parsed.optional,
                optional: parsed.optional,
                doc_str: "".to_string(),
                prop_type,
            });
            file_dependencies.push(File {
                path: std::path::PathBuf::from(format!("{}/{}.dart", name, sanitized_p_name)),
                content: parsed.content,
            });
            for f in parsed.files.into_iter() {
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
                    String::from("?.toJson()")
                } else {
                    match &prop.prop_type {
                        PropertyType::Primitive(PrimitivePropertyType::List {
                            inner_is_primitive,
                            ..
                        }) => {
                            format!(
                                "?.map((e) => {}).toList()",
                                if *inner_is_primitive {
                                    "e"
                                } else {
                                    "e.toJson()"
                                }
                            )
                        }
                        _ => "".to_string(),
                    }
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
                            format!("((val){{ if (val is {}{}) return val; throw BEAMWrongTypeError('$val is not of type {} for property {}'); }})(json['{}'])", prop.typ, if prop.nullable { "?" } else { "" }, prop.typ, prop.name, prop.name)
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
        GeneratedCode {
            content,
            files: file_dependencies,
        }
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
    name.chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect()
}

/// Returns the English word for `c` if it is an ASCII digit.
fn digit_to_word(c: char) -> Option<&'static str> {
    match c {
        '0' => Some("zero"),
        '1' => Some("one"),
        '2' => Some("two"),
        '3' => Some("three"),
        '4' => Some("four"),
        '5' => Some("five"),
        '6' => Some("six"),
        '7' => Some("seven"),
        '8' => Some("eight"),
        '9' => Some("nine"),
        _ => None,
    }
}

/// Like [`sanitize`], but additionally ensures the result is a valid Dart
/// identifier on its own by replacing a leading ASCII digit with its written
/// variant (e.g. `3` -> `three`, `3d` -> `three_d`, `404Error` ->
/// `four04Error`). Use this whenever the produced string stands alone as a
/// Dart identifier (field, getter, method name); plain [`sanitize`] is fine
/// for fragments that are concatenated with a non-digit prefix.
pub fn sanitize_identifier(name: &str) -> String {
    let sanitized = sanitize(name);
    if let Some(word) = sanitized.chars().next().and_then(digit_to_word) {
        let rest = &sanitized[1..];
        return if rest.is_empty() {
            word.to_string()
        } else {
            format!("{}_{}", word, rest)
        };
    }
    sanitized
}

pub fn create_property_name(name: &str) -> String {
    let sanitized = sanitize_identifier(name);
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

/// Result of [`SchemeAdder::parse_named_iast`].
///
/// `content` is the generated Dart source for this node, `files` are the
/// dependent files produced while recursing, `special_case` carries the
/// classification (link / list / primitive) used by callers to decide how
/// to reference this type, and `nullable` / `optional` / `is_binary`
/// propagate the corresponding annotations.
pub(super) struct ParsedIast {
    pub content: String,
    pub files: Vec<File>,
    pub special_case: Option<GenerationSpecialCase>,
    pub nullable: bool,
    pub optional: bool,
    pub is_binary: bool,
}

/// Result of [`SchemeAdder::generate_primitive_sum_type`]: the generated
/// Dart `enum` source together with the name of the enum class so callers
/// can reference it.
pub(super) struct EnumCode {
    pub class_name: String,
    pub content: String,
}

/// A single permitted value of an enum, as consumed by
/// [`SchemeAdder::generate_primitive_sum_type`]. `is_string` controls
/// whether the value is emitted quoted in the generated Dart.
pub(super) struct AllowedValue<'a> {
    pub value: &'a str,
    pub is_string: bool,
    pub description: &'a str,
}

/// One arm of a discriminated union built by
/// [`SchemeAdder::generate_discriminated_sum_type`]: the generated wrapper
/// `class_name`, the `discriminator_value` that selects it, and the
/// `referenced_type_name` it wraps.
struct DiscriminatedVariant<'a> {
    class_name: String,
    discriminator_value: &'a str,
    referenced_type_name: String,
}

/// One arm of a (non-discriminated) union built by
/// [`SchemeAdder::generate_sum_type`]: the generated wrapper `class_name`
/// and its `special_case` classification (link / list / primitive), used
/// to decide how each variant is (de)serialized.
struct SumVariantClass {
    class_name: String,
    special_case: Option<GenerationSpecialCase>,
}

/// One arm of a response union built by
/// [`SchemeAdder::generate_response_union`]. Each arm corresponds to a
/// single HTTP status `code` and wraps a value of `value_type`, decoded
/// according to `is_primitive` / `list_inner_type` (mirroring the
/// classification carried by the endpoint generator's `ResponseClass`).
/// `import_path` is the sibling per-code schema file the union imports and
/// re-exports.
pub(super) struct ResponseUnionVariant {
    pub code: String,
    pub value_type: String,
    pub is_primitive: bool,
    pub list_inner_type: Option<String>,
    pub import_path: String,
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
