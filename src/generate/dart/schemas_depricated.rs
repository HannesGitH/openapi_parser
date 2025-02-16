use oas3::spec as oas3spec;

use super::super::interface::*;
use super::types;

pub(super) struct SchemaAdder<'a> {
    class_prefix: &'a str,
}

struct UnionTypeString {
    name: String,
    prefix: String,
}

impl<'a> SchemaAdder<'a> {
    pub(super) fn new(class_prefix: &'a str) -> Self {
        Self { class_prefix }
    }

    pub(super) fn add_schemas(&self, out: &mut Vec<File>, spec: &oas3::Spec) {
        let mut scheme_files = Vec::new();
        for component in spec.components.iter() {
            for (name, schema_obj) in component.schemas.iter() {
                let schema = match schema_obj {
                    oas3spec::ObjectOrReference::Object(schema) => schema,
                    oas3spec::ObjectOrReference::Ref { ref_path } => {
                        println!("root ref spec currently not supported: {}", ref_path);
                        continue;
                    }
                };
                let (content, depends_on_files) = self.mk_schema_file(name, schema, 0);
                scheme_files.push(File {
                    path: std::path::PathBuf::from(format!("{}.dart", name)),
                    content: content,
                });
                scheme_files.extend(depends_on_files);
            }
        }
        out.push(File {
            path: std::path::PathBuf::from("schemes/schemes.dart"),
            content: {
                let mut content = String::new();
                for file in scheme_files.iter() {
                    content.push_str(&format!("export '{}';\n", file.path.display()));
                }
                content
            },
        });
        out.extend(scheme_files.into_iter().map(|f| File {
            path: std::path::PathBuf::from(format!("schemes/{}", f.path.display())),
            content: f.content,
        }));
    }

    fn scheme_name_to_type(&self, name: &str) -> String {
        format!("{}{}Scheme", self.class_prefix, name)
    }

    fn mk_union_type_string(&self, name: &str, inner: &Vec<types::NnDt>) -> UnionTypeString {
        let class_name = format!("{}UnionType", name);
        let mut prefix = String::new();
        prefix.push_str(&format!("sealed class {} {{}}\n", class_name));
        for (idx, t) in inner.iter().enumerate() {
            prefix.push_str(&format!(
                "class {}{}{} extends {}{{}}\n",
                class_name, t, idx, class_name
            ));
        }
        UnionTypeString {
            name: class_name,
            prefix,
        }
    }
    fn mk_schema_file(
        &self,
        name: &str,
        scheme: &oas3spec::ObjectSchema,
        depth: usize,
    ) -> (String, Vec<File>) {
        let mut depends_on_files = Vec::new();
        let mut content = String::new();
        let mut prefix_content = String::new();
        let mut constructor_content = String::new();
        let class_name = self.scheme_name_to_type(name);
        // prefix_content.push_str(&format!("import '{0}schemes.dart';\n\n", "../".repeat(depth)));
        // let any_of_name = String::from("any_of");
        content.push_str(&format!("\nclass {} {{\n", class_name));
        let properties = &scheme
            .properties;
            // .iter()
            // .chain(scheme.any_of.iter().map(|p| (&any_of_name, p)))
            // .collect::<Vec<_>>();
        let iter = properties;
        if !iter.is_empty() {
            constructor_content.push_str(&format!("  {}({{\n", class_name));
            for (p_name, p_scheme) in iter.iter() {
                match p_scheme {
                    oas3spec::ObjectOrReference::Object(p_scheme) => {
                        if let Some(schema_type) = &p_scheme.schema_type {
                            let dart_type = scheme_type_to_dart_type(schema_type);

                            let dt_str = self.create_dart_type_string(
                                &dart_type,
                                &class_name,
                                &mut prefix_content,
                            );

                            add_to_constructor_content(
                                &mut constructor_content,
                                p_name,
                                matches!(dart_type, types::Dt::Normal(_)),
                            );

                            add_to_member_content(&mut content, p_name, &dt_str);
                        } else if !p_scheme.any_of.is_empty() {
                            let list = &p_scheme.any_of;
                            let scheme_types = list.iter().map(|obj_or_ref| match obj_or_ref {
                                oas3spec::ObjectOrReference::Object(obj) => match &obj.schema_type {
                                    Some(schema_type) => match schema_type {
                                        oas3spec::SchemaTypeSet::Single(schema_type) => schema_type.clone(),
                                        _ => panic!("Multiple schema types not supported in any_of"),
                                    },
                                    None => panic!("No schema type found in any_of"),
                                },
                                _ => panic!("References not supported in any_of")
                            }).collect::<Vec<_>>();
                            let dt = multiple_scheme_type_to_dart_type(&scheme_types);
                            let dt_str =
                                self.create_dart_type_string(&dt, &class_name, &mut prefix_content);
                            add_to_constructor_content(&mut constructor_content, p_name, true);
                            add_to_member_content(&mut content, p_name, &dt_str);
                        } else {
                            depends_on_files.push(File {
                                path: std::path::PathBuf::from(format!("{}/{}.dart", name, p_name)),
                                content: self.mk_schema_file(p_name, p_scheme, depth + 1).0,
                            });

                            let dt_str = self.scheme_name_to_type(p_name);
                            add_to_constructor_content(&mut constructor_content, p_name, true);
                            add_to_member_content(&mut content, p_name, &dt_str);
                        }
                    }
                    oas3spec::ObjectOrReference::Ref { ref_path } => {
                        println!("ref path: {}", ref_path);
                    }
                }
            }
            constructor_content.push_str(&format!("  }});\n"));
        }
        let mut ret_str = String::new();
        for file in depends_on_files.iter() {
            ret_str.push_str(&format!("import '{0}';\n", file.path.display()));
        }
        ret_str.push_str(&prefix_content);
        ret_str.push_str(&content);
        ret_str.push_str(&constructor_content);
        ret_str.push_str(&format!("}}\n"));
        (ret_str, depends_on_files)
    }

    fn create_dart_type_string(
        &self,
        dart_type: &types::Dt,
        parent_type: &str,
        sealed_class_str_buf: &mut String,
    ) -> String {
        use types::*;
        let mut inner_str = |nn_dt: &NnDt| match nn_dt {
            NnDt::Builtin(dt) => format!("{}", dt),
            NnDt::Union(inner) => {
                let uts = self.mk_union_type_string(parent_type, inner);
                sealed_class_str_buf.push_str(&uts.prefix);
                uts.name
            }
        };
        let dt_str = match dart_type {
            Dt::Normal(dt) => inner_str(&dt),
            Dt::Nullable(dt) => format!("{}?", inner_str(&dt)),
        };
        dt_str
    }
}

// SECTION: dart content string builders

fn add_to_constructor_content(content: &mut String, name: &str, is_required: bool) {
    content.push_str(&format!(
        "    {}this.{},\n",
        if is_required { "required " } else { "" },
        name
    ));
}

fn add_to_member_content(content: &mut String, name: &str, type_str: &str) {
    content.push_str(&format!("    final {} {};\n", type_str, name));
}

// SECTION: type conversion

fn scheme_type_to_dart_type(schema_type: &oas3spec::SchemaTypeSet) -> types::Dt {
    type OaSt = oas3spec::SchemaTypeSet;
    use types::*;
    match schema_type {
        OaSt::Single(schema_type) => {
            let inner = single_scheme_type_to_dart_type(schema_type);
            Dt::Normal(NnDt::Builtin(inner))
        }
        OaSt::Multiple(schema_types) => multiple_scheme_type_to_dart_type(schema_types),
    }
}

fn multiple_scheme_type_to_dart_type(schema_types: &Vec<oas3spec::SchemaType>) -> types::Dt {
    use types::*;
    let builtins = schema_types
        .iter()
        .map(|t| single_scheme_type_to_dart_type(t))
        .collect::<Vec<types::BiDt>>();
    // check if inner contains BiDt::Never
    if builtins.contains(&BiDt::Never) {
        // remove BiDt::Never from inner
        let mut non_null_types = Vec::new();
        for t in builtins {
            if t != BiDt::Never {
                non_null_types.push(NnDt::Builtin(t));
            }
        }
        let inner = if non_null_types.len() == 1 {
            non_null_types.swap_remove(0)
        } else {
            NnDt::Union(non_null_types)
        };
        Dt::Nullable(inner)
    } else {
        let unwrapped = builtins.into_iter().map(|t| NnDt::Builtin(t)).collect();
        Dt::Normal(NnDt::Union(unwrapped))
    }
}

fn single_scheme_type_to_dart_type(schema_type: &oas3spec::SchemaType) -> types::BiDt {
    type OaSt = oas3spec::SchemaType;
    use types::*;
    match schema_type {
        OaSt::String => BiDt::String,
        OaSt::Number => BiDt::Number,
        OaSt::Boolean => BiDt::Boolean,
        OaSt::Integer => BiDt::Integer,
        OaSt::Object => BiDt::Object,
        OaSt::Array => BiDt::List(Box::new(Dt::Normal(NnDt::Builtin(BiDt::Object)))),
        OaSt::Null => BiDt::Never,
    }
}
