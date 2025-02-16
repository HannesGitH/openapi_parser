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

        for (idx, iast) in sum.iter().enumerate() {
            let (content, depends_on_files) =
                self.parse_named_iast(&format!("{}_{}", name, idx), iast, depth + 1);
            dependencies.push(File {
                path: std::path::PathBuf::from(format!("{}/{}.dart", name, idx)),
                content,
            });
            for f in depends_on_files.into_iter() {
                dependencies.push(File {
                    path: std::path::PathBuf::from(format!("{}/{}", name, f.path.display())),
                    content: f.content,
                });
            }
        }

        let mut content = String::new();
        content.push_str(&format!(
            "{}sealed class {}Union {{\n}}",
            doc_str, class_name
        ));
        //TODO: add sub-classes
        // // for f in dependencies.iter() {
        // //     content.push_str(&format!(
        // //         "{} class {} extends {}Union {{\n}}",
        // //         doc_str, class_name, f.path.display()
        // //     ));
        // // };
        (content, dependencies)
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
        let mut properties: Vec<Property> = Vec::new();

        for (p_name, iast) in product.iter() {
            if let intermediate::types::IAST::Primitive(prim) = &iast {
                properties.push(Property {
                    name: p_name,
                    typ: to_dart_prim(&prim.value),
                    nullable: prim.nullable,
                    doc_str: mk_doc_str(p_name, &prim, 1),
                });
                continue;
            }
            let full_name = format!("{}_{}", name, p_name);
            let (content, depends_on_files) = self.parse_named_iast(&full_name, iast, depth + 1);
            properties.push(Property {
                name: p_name,
                typ: full_name,
                nullable: false,
                doc_str: "".to_string(),
            });
            file_dependencies.push(File {
                path: std::path::PathBuf::from(format!("{}/{}.dart", name, p_name)),
                content,
            });
            for f in depends_on_files.into_iter() {
                file_dependencies.push(File {
                    path: std::path::PathBuf::from(format!("{}/{}", name, f.path.display())),
                    content: f.content,
                });
            }
        }

        let mut content = String::new();
        //TODO: import dependencies
        content.push_str(&format!("{}class {} {{\n", doc_str, class_name));
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
        content.push_str("\n}");
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
        doc_str.push_str(&format!("{}/// {}\n", "\t".repeat(tabs), description));
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
}
