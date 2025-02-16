use std::collections::HashMap;

use super::super::interface::*;

use crate::parse::intermediate;

pub(super) struct SchemeAdder<'a> {
    class_prefix: &'a str,
    class_suffix: &'a str,
}

impl<'a> SchemeAdder<'a> {
    pub(super) fn new(class_prefix: &'a str, class_suffix: &'a str) -> Self {
        Self {
            class_prefix,
            class_suffix,
        }
    }

    pub(super) fn add_schemes(
        &self,
        out: &mut Vec<File>,
        intermediate: &intermediate::IntermediateFormat,
    ) {
        let mut scheme_files = Vec::new();
        for scheme in intermediate.schemes.iter() {
            let (content, depends_on_files) = self.parse_named_iast(scheme.name, &scheme.obj, 0);
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

    fn class_name(&self, name: &str) -> String {
        format!("{}{}{}", self.class_prefix, name, self.class_suffix)
    }

    fn parse_named_iast(
        &self,
        name: &str,
        iast: &intermediate::IAST,
        depth: usize,
    ) -> (String, Vec<File>) {
        match iast {
            intermediate::IAST::Object(annotated_obj) => {
                let doc_str = mk_doc_str(name, annotated_obj, 0);
                let alg_type = &annotated_obj.value;
                use intermediate::AlgType;
                match alg_type {
                    AlgType::Sum(sum) => self.generate_sum_type(name, &doc_str, sum, depth),
                    AlgType::Product(product) => {
                        self.generate_product_type(name, &doc_str, product, depth)
                    }
                }
            }
            intermediate::IAST::Reference(_) => todo!(),
            intermediate::IAST::Primitive(annotated_obj) => {
                let doc_str = mk_doc_str(name, annotated_obj, 0);
                (
                    format!(
                        "{}typedef {} = {};",
                        doc_str,
                        self.class_name(name),
                        to_dart_prim(&annotated_obj.value)
                    ),
                    vec![],
                )
            }
        }
    }

    fn generate_sum_type(
        &self,
        name: &str,
        doc_str: &str,
        sum: &Vec<intermediate::IAST>,
        depth: usize,
    ) -> (String, Vec<File>) {
        let class_name = self.class_name(name);
        let mut dependencies = Vec::new();

        let index_to_name = |idx: usize| format!("{}{}", name, idx);

        let mut variants = Vec::new();

        for (idx, iast) in sum.iter().enumerate() {
            let variant_name = index_to_name(idx);
            let (content, depends_on_files) = self.parse_named_iast(&variant_name, iast, depth + 1);
            dependencies.push(File {
                path: std::path::PathBuf::from(format!("{}/{}.dart", name, idx)),
                content,
            });
            variants.push(self.class_name(&variant_name));
            for f in depends_on_files.into_iter() {
                dependencies.push(File {
                    path: std::path::PathBuf::from(format!("{}/{}", name, f.path.display())),
                    content: f.content,
                });
            }
        }

        let mut content = String::new();

        content.push_str(&format!(
            "import '../{}utils/serde.dart';\n",
            "../".repeat(depth)
        ));

        for f in dependencies.iter() {
            content.push_str(&format!("import '{}';\n", f.path.display()));
        }

        content.push_str(&format!(
            "{}sealed class {} implements APISerde {{\n\tconst {}();",
            doc_str, class_name, class_name
        ));
        content.push_str("\n\n\t@Deprecated(\"not deprecated, but usage is highly discouraged, as its not deterministic\")");
        content.push_str(&format!("\n\tfactory {}.fromJson(dynamic json) {{", class_name));
        for v in variants.iter() {
            content.push_str(&format!("\n\t\ttry{{\n\t\t\treturn {}_.fromJson(json);\n\t\t}} catch(e) {{}}", v));
        }
        content.push_str(&format!("\n\t\tthrow Exception('Could not parse json into {}');\n\t}}", class_name));

        content.push_str("\n}\n\n");

        for v in variants.iter() {
            content.push_str(&format!(
                "class {}_ extends {} {{\n",
                v, class_name
            ));
            content.push_str(&format!("  final {} value;\n", v));
            content.push_str(&format!("  const {}_(this.value);\n", v));
            content.push_str(&format!(
                "\n  @override\n  dynamic toJson() => value.toJson();\n"
            ));
            content.push_str(&format!(
                "  factory {}_.fromJson(dynamic json) => \n\t\t{}_({}.fromJson(json));\n",
                v, v, v
            ));
            content.push_str("}\n\n");
        }

        (content, dependencies)
    }

    /// this is an enum
    fn generate_primitive_sum_type(
        &self,
        name: &str,
        doc_str: &str,
        allowed_values: &Vec<String>,
    ) -> (String, String) {
        let class_name = format!("{}{}", self.class_prefix, name);
        let allowed_values_str = allowed_values
            .iter()
            .map(|v| {
                (
                    v,
                    v.chars()
                        .filter(|c| c.is_alphanumeric())
                        .collect::<String>(),
                )
            })
            .collect::<Vec<(_, _)>>();
        let mut content = String::new();
        content.push_str(&format!(
            "\n{}enum {} implements APISerde {{\n",
            doc_str, class_name
        ));
        for (orig_value, enum_value) in allowed_values_str.iter() {
            content.push_str(&format!("  ///{}\n", orig_value));
            content.push_str(&format!("  t_{},\n", enum_value));
        }
        content.push_str(&"\t;\n\n\t@override\n\tdynamic toJson() => switch(this) {\n");
        for (orig_value, enum_value) in allowed_values_str.iter() {
            content.push_str(&format!("\t\tt_{} => '{}',\n", enum_value, orig_value));
        }
        content.push_str(&format!(
            "\t}};\n\tfactory {}fromJson(dynamic json) => switch(json) {{\n",
            class_name
        ));
        for (orig_value, enum_value) in allowed_values_str.iter() {
            content.push_str(&format!("\t\t'{}' => t_{},\n", orig_value, enum_value));
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

        for (p_name, iast) in product.iter() {
            if let intermediate::IAST::Primitive(prim) = &iast {
                let prim_type = match &prim.value {
                    intermediate::types::Primitive::Enum(allowed_values) => {
                        let full_name = format!("{}_{}", name, p_name);
                        let (class_name, content) =
                            self.generate_primitive_sum_type(&full_name, doc_str, &allowed_values);
                        extra_content.push_str(&content);
                        class_name
                    }
                    intermediate::types::Primitive::List(inner_iast) => {
                        let full_name = format!("{}_{}", name, p_name);
                        let (content, depends_on_files) =
                            self.parse_named_iast(&full_name, inner_iast, depth + 1);
                        file_dependencies.push(File {
                            path: std::path::PathBuf::from(format!("{}/{}.dart", name, p_name)),
                            content,
                        });
                        for f in depends_on_files.into_iter() {
                            file_sub_dependencies.push(f);
                        }
                        format!(
                            "{}<{}>",
                            to_dart_prim(&prim.value),
                            self.class_name(&full_name)
                        )
                    }
                    intermediate::types::Primitive::Map(inner_iast) => {
                        let full_name = format!("{}_{}", name, p_name);
                        let (content, depends_on_files) =
                            self.parse_named_iast(&full_name, inner_iast, depth + 1);
                        file_dependencies.push(File {
                            path: std::path::PathBuf::from(format!("{}/{}.dart", name, p_name)),
                            content,
                        });
                        for f in depends_on_files.into_iter() {
                            file_sub_dependencies.push(f);
                        }
                        format!(
                            "{}<String,{}>",
                            to_dart_prim(&prim.value),
                            self.class_name(&full_name)
                        )
                    }
                    _ => to_dart_prim(&prim.value),
                };
                properties.push(Property {
                    name: p_name,
                    typ: prim_type,
                    nullable: prim.nullable,
                    doc_str: mk_doc_str(p_name, &prim, 1),
                    is_primitive: true,
                });
                continue;
            }
            let full_name = format!("{}_{}", name, p_name);
            let (content, depends_on_files) = self.parse_named_iast(&full_name, iast, depth + 1);
            properties.push(Property {
                name: p_name,
                typ: self.class_name(&full_name),
                nullable: false,
                doc_str: "".to_string(),
                is_primitive: false,
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
        }
        content.push_str("\n\n");

        content.push_str(&format!(
            "{}class {} implements APISerde {{\n",
            doc_str, class_name
        ));
        for prop in properties.iter() {
            content.push_str(&format!(
                "\n{}  final {}{} {};\n",
                prop.doc_str,
                prop.typ,
                if prop.nullable { "?" } else { "" },
                prop.name
            ));
        }

        // constructor
        content.push_str(&format!("\n\n  const {}({{\n", class_name));
        for prop in properties.iter() {
            content.push_str(&format!(
                "    {}this.{},\n",
                if !prop.nullable { "required " } else { "" },
                prop.name
            ));
        }
        content.push_str("  });\n");

        //to json
        content.push_str("\n\n  @override\n  Map<String,dynamic> toJson() => {\n");
        for prop in properties.iter() {
            content.push_str(&format!(
                "    {}'{}': {}{},\n",
                if prop.nullable {
                    format!("if({} != null) ", prop.name)
                } else {
                    "".to_string()
                },
                prop.name,
                prop.name,
                if !prop.is_primitive { ".toJson()" } else { "" }
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
                prop.name,
                if prop.is_primitive {
                    format!("json['{}']", prop.name)
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

fn to_dart_prim(primitive: &intermediate::types::Primitive) -> String {
    match primitive {
        intermediate::types::Primitive::String => "String".to_string(),
        intermediate::types::Primitive::Number => "num".to_string(),
        intermediate::types::Primitive::Integer => "int".to_string(),
        intermediate::types::Primitive::Boolean => "bool".to_string(),
        intermediate::types::Primitive::Never => "Never".to_string(),
        intermediate::types::Primitive::List(_) => "List".to_string(),
        intermediate::types::Primitive::Map(_) => "Map".to_string(),
        intermediate::types::Primitive::Enum(_) => "Enum".to_string(),
        intermediate::types::Primitive::Dynamic => "dynamic".to_string(),
    }
}

struct Property<'a> {
    name: &'a str,
    typ: String,
    nullable: bool,
    doc_str: String,
    is_primitive: bool,
}
