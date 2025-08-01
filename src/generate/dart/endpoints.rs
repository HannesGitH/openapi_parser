use crate::{
    generate::{dart::schemes::{sanitize, GenerationSpecialCaseType}, File},
    parse::intermediate::{self, Route, RouteFragmentLeafData},
};

use super::schemes;

#[macro_use]
mod macros;
pub struct EndpointAdder<'a> {
    scheme_adder: &'a schemes::SchemeAdder<'a>,
    intermediate: &'a intermediate::IntermediateFormat<'a>,
}

impl<'a> EndpointAdder<'a> {
    pub fn new(
        scheme_adder: &'a schemes::SchemeAdder<'a>,
        intermediate: &'a intermediate::IntermediateFormat<'a>,
    ) -> Self {
        Self {
            scheme_adder,
            intermediate,
        }
    }
    pub fn add_endpoints(&self, out: &mut Vec<File>) {
        let intermediate = self.intermediate;
        let mut out_files: Vec<File> = Vec::new();
        let interface_content = include_str!("endpoints/interface.dart");
        let (path_enum_name, paths_enum_content) = self.scheme_adder.generate_primitive_sum_type(
            "Paths",
            "",
            &intermediate
                .routes
                .iter()
                // we always have a string path, so we can always use true here (i guess)
                .map(|r| (r.path, true, r.description.unwrap_or("")))
                .collect::<Vec<(&str, bool, &str)>>(),
        );

        for route in &intermediate.routes {
            let sanitized_path = sanitize(route.path);
            let name = format!("{}Methods", sanitized_path);
            let (content, files) = self.generate_path_method_wrapper(&name, route, 1);
            out_files.push(File {
                path: std::path::PathBuf::from(format!("routes/{}.dart", sanitized_path)),
                content,
            });
            out_files.extend(files.into_iter().map(|f| File {
                path: std::path::PathBuf::from(format!("routes/{}", f.path.to_str().unwrap())),
                content: f.content,
            }));
        }

        let (frag_class_name, frag_content, frag_deps) = self.generate_route_fragment(
            "root",
            &intermediate.routes_tree,
            &intermediate.routes,
            true,
            0,
        );
        let root_frag_file_name = "root_fragment.dart";
        out_files.push(File {
            path: std::path::PathBuf::from(root_frag_file_name),
            content: frag_content,
        });
        out_files.extend(frag_deps);

        let mut imports_content = String::new();
        imports_content.push_str(&include_str!("endpoints/imports.dart"));
        cpf!(imports_content, "import '{}';", root_frag_file_name);
        let mut content = String::new();
        content.push_str(&imports_content);
        content.push_str(&format!("typedef BEAMPathEnum={};\n", path_enum_name));
        content.push_str(&paths_enum_content);
        content.push_str(&interface_content);
        out_files.push(File {
            path: std::path::PathBuf::from("endpoints.dart"),
            content,
        });
        out.extend(out_files.into_iter().map(|f| File {
            path: std::path::PathBuf::from(format!("endpoints/{}", f.path.to_str().unwrap())),
            content: f.content,
        }));
    }

    fn generate_path_method_wrapper(
        &self,
        name: &str,
        route: &Route,
        depth: usize,
    ) -> (String, Vec<File>) {
        let mut deps: Vec<File> = Vec::new();
        let mut imports_str = String::new();
        let mut c = String::new();
        let mut param_typedef_strs = String::new();
        // let mut all_ret_types_str = String::new();
        cpf!(imports_str, "// ignore_for_file: unused_import");
        cpf!(
            imports_str,
            "import '{}endpoints.dart';",
            "../".repeat(depth)
        );
        cpf!(
            imports_str,
            "import '{}utils/serde.dart';",
            "../".repeat(depth + 1)
        );
        cpf!(c, "/// {}", route.path);
        cpf!(
            c,
            "/// {}",
            route.description.unwrap_or("").replace("\n", "\n/// ")
        );
        cpf!(c, "class BEAM{} extends BEAMPath {{", name);
        cpf!(
            c,
            "  BEAM{}({{required super.interpolatedPath, required super.handler}})",
            name
        );
        cpf!(
            c,
            "    :super(path: BEAMPathEnum.fromJson('{}'));",
            route.path
        );
        for method in &route.endpoints {
            let method_str = method.method.string();
            let param_name = format!("_P_{}", method_str);
            let (body_str, body_is_primitive, body_list_inner_type) = match &method.request {
                Some(request) => {
                    //do things
                    let request_name = format!("{}{}Request", name, method_str);
                    let (content, sub_deps, special_case, nullable, optional) = self
                        .scheme_adder
                        .parse_named_iast(&request_name, request, depth + 1);
                    deps.extend(sub_deps.into_iter().map(|f| File {
                        path: std::path::PathBuf::from(format!(
                            "{}/{}",
                            name,
                            f.path.to_str().unwrap()
                        )),
                        content: f.content,
                    }));
                    let dep_path_str = format!("{}/{}.req.body.schema.dart", name, method_str);
                    let dep_path = std::path::PathBuf::from(&dep_path_str);
                    deps.push(File {
                        path: dep_path,
                        content,
                    });
                    imports_str.push_str(&format!("import '{}';\n", &dep_path_str));
                    imports_str.push_str(&format!("export '{}';\n", &dep_path_str));
                    let body_type_str = match special_case {
                        Some(schemes::GenerationSpecialCase { reason, type_name }) => {
                            (
                                type_name, 
                                match reason {
                                    GenerationSpecialCaseType::List(_, is_primitive) => true,
                                    GenerationSpecialCaseType::Primitive => true,
                                    //TODO: might also be a primitive link
                                    GenerationSpecialCaseType::Link(_) => false,
                                },
                                match reason {
                                    GenerationSpecialCaseType::List(inner_type, _) => Some(inner_type),
                                    _ => None,
                                },
                            )
                        }
                        _ => (self.scheme_adder.class_name(&request_name), false, None),
                    };
                    (
                        Some(format!(" {{required {} body}}", body_type_str.0)),
                        body_type_str.1,
                        body_type_str.2,
                    )
                }
                None => (None, true, None),
            };
            let params_str = if method.params.is_empty() {
                String::new()
            } else {
                format!("{} params,", param_name)
            };
            let (params_1typedef_str, params_as_json_body_str) =
                &mk_params(&method.params, &param_name);

            let (ret_type_str, ret_is_primitive, ret_list_inner_type) = {
                let responses = &method.responses;
                if responses.is_empty() {
                    ("()".to_string(), true, None)
                } else {
                    let response_name = format!("{}_{}Response", name, method_str);
                    if responses.len() == 1 {
                        let (response_code, response) = responses.first_key_value().unwrap();
                        let (content, sub_deps, special_case, nullable, optional) = self
                            .scheme_adder
                            .parse_named_iast(&response_name, &response, depth + 1);
                        let dep_path_str =
                            format!("{}/{}.resp.{}.schema.dart", name, method_str, response_code);
                        let dep_path = std::path::PathBuf::from(&dep_path_str);
                        deps.push(File {
                            path: dep_path,
                            content: content,
                        });
                        deps.extend(sub_deps.into_iter().map(|f| File {
                            path: std::path::PathBuf::from(format!(
                                "{}/{}",
                                name,
                                f.path.to_str().unwrap()
                            )),
                            content: f.content,
                        }));
                        imports_str.push_str(&format!("import '{}';\n", &dep_path_str));
                        imports_str.push_str(&format!("export '{}';\n", &dep_path_str));
                        match special_case {
                            Some(schemes::GenerationSpecialCase { reason, type_name }) => (
                                type_name,
                                reason == GenerationSpecialCaseType::Primitive
                                    || {
                                        // assozialer edge case wo wir gelinkt haben aber auf einen primitive type
                                        // TODO: do this in a better (not just-one-edge-case-fix kinda) way
                                        if let GenerationSpecialCaseType::Link(link) = &reason {
                                            use crate::parse::intermediate::types::*;
                                            if let Some(Scheme {
                                                obj: IAST::Primitive(_),
                                                ..
                                            }) = self
                                                .intermediate
                                                .schemes
                                                .iter()
                                                .find(|s| s.name == link)
                                            {
                                                true
                                            } else {
                                                false
                                            }
                                        } else {
                                            false
                                        }
                                    }
                                    || if let GenerationSpecialCaseType::List(_, is_primitive) =
                                        &reason
                                    {
                                        is_primitive.clone()
                                    } else {
                                        false
                                    },
                                if let GenerationSpecialCaseType::List(inner_type, _) = reason {
                                    Some(inner_type)
                                } else {
                                    None
                                },
                            ),
                            None => (self.scheme_adder.class_name(&response_name), false, None),
                        }
                    } else {
                        (
                            "//TODO: add parser for multiple responses".to_string(),
                            false,
                            None,
                        )
                    }
                }
            };
            let impl_str = {
                let mut s = String::new();
                s.push_str(&format!(
                    "\n\t\t{}",
                    params_as_json_body_str.replace("\n", "\n\t\t")
                ));
                cpf!(s, "return handle(method: BEAMRequestMethod.{}, params: paramsJson, body: {}).then((json) => {});", method_str,match (&body_str, &body_is_primitive) {
                    (Some(_), true) => "body",
                    (Some(_), false) => "body?.toJson()",
                    (None, _) => "null",
                }, match (ret_is_primitive, ret_list_inner_type) {
                    (true, None) => "json".to_string(),
                    (true, Some(inner_type)) => format!("json"),
                    (false, Some(inner_type)) => format!("(json as List).map((e) => {}.fromJson(e)).toList()", inner_type),
                    (false, None) => format!("{}.fromJson(json)", ret_type_str),
                });
                s
            };

            param_typedef_strs.push_str(&params_1typedef_str);
            cpf!(
                c,
                "\n\t///{}\n\t///",
                method.summary.unwrap_or("").replace("\n", "\n\t/// ")
            );
            cpf!(
                c,
                "\t///{}",
                method.description.unwrap_or("").replace("\n", "\n\t/// ")
            );
            cpf!(
                c,
                "  Future<{}> {}({}{}){{{}\t}}",
                ret_type_str,
                method_str,
                params_str,
                match &body_str {
                    Some(body_str) => body_str,
                    None => "",
                },
                impl_str
            );
        }
        cpf!(c, "}}\n");
        c.push_str(&param_typedef_strs);
        // c.push_str(&all_ret_types_str);
        imports_str.push_str(&c);
        (imports_str, deps)
    }

    /// returns its own class name, the content to build it and the files it depends on
    fn generate_route_fragment(
        &self,
        name: &str,
        fragment: &intermediate::RouteFragment,
        routes: &[Route],
        is_root: bool,
        depth: usize,
    ) -> (String, String, Vec<File>) {
        let name = sanitize(name);
        let mut s = String::new();
        let mut deps = Vec::new();
        let mut imports_str = String::new();

        use intermediate::RouteFragment;
        let mut class_name = format!("BEAM{}Frag_{}", name, "unknown");
        match fragment {
            RouteFragment::Node(node) => {
                cpf!(
                    imports_str,
                    "import '{}endpoints.dart';",
                    "../".repeat(depth)
                );
                let sanitized_frag_name = sanitize(node.path_fragment_name.as_str());
                class_name = format!("BEAM{}Frag_{}", name, sanitized_frag_name);

                let sub_dir_name = format!("{}_frags", node.path_fragment_name);
                cpf!(s, "class {} extends BEAMWithParent {{", class_name);
                cpf!(
                    s,
                    "\t{}({{required super.deps, required super.parent{}}}) : super(ownFragment: {});\n",
                    class_name,
                    if node.is_param {
                        ", required String param"
                    } else {
                        ""
                    },
                    if node.is_param {
                        "param".to_string()
                    } else {
                        format!("'{}'", node.path_fragment_name)
                    }
                );
                if is_root {
                    cpf!(s, "  @override\n\tString get path => '';");
                }
                for child in &node.children {
                    let child_name = format!("{}_{}", name, node.path_fragment_name);
                    let (child_class_name, child_str, child_deps) =
                        self.generate_route_fragment(&child_name, child, routes, false, depth + 1);
                    let child_file_name = format!(
                        "{}/{}.dart",
                        sub_dir_name,
                        match child {
                            intermediate::RouteFragment::Node(child_node) => {
                                let child_frag_name = &child_node.path_fragment_name;
                                let sanitized_child_frag_name = sanitize(child_frag_name);
                                if child_node.is_param {
                                    cpf!(
                                        s,
                                        "\t{} {}(String param) => {}(parent: this, param: param, deps: this.deps);",
                                        child_class_name,
                                        sanitized_child_frag_name,
                                        child_class_name
                                    );
                                } else {
                                    cpf!(
                                        s,
                                        "\t{} get {} => {}(parent: this, deps: this.deps);",
                                        child_class_name,
                                        sanitized_child_frag_name,
                                        child_class_name
                                    );
                                };
                                child_frag_name
                            }
                            intermediate::RouteFragment::Leaf(_) => {
                                cpf!(
                                    s,
                                    "\t{} call() => {}(interpolatedPath: this.path, handler: this.deps);",
                                    child_class_name,
                                    child_class_name
                                );
                                "leaf"
                            }
                        }
                    );
                    let sub_frag_file = File {
                        path: std::path::PathBuf::from(&child_file_name),
                        content: child_str,
                    };
                    deps.extend(child_deps.into_iter().map(|f| File {
                        path: std::path::PathBuf::from(format!(
                            "{}/{}",
                            sub_dir_name,
                            f.path.to_str().unwrap()
                        )),
                        content: f.content,
                    }));
                    deps.push(sub_frag_file);
                    imports_str.push_str(&format!("import '{}';\n", child_file_name));
                    imports_str.push_str(&format!("export '{}';\n", child_file_name));
                }
                cpf!(s, "}}\n");
            }
            RouteFragment::Leaf(RouteFragmentLeafData { route_idx }) => {
                let route = &routes[*route_idx];
                let sanitized_route_str = sanitize(route.path);
                s.push_str(&format!(
                    "export '{}routes/{}.dart';",
                    "../".repeat(depth),
                    sanitized_route_str
                ));
                class_name = format!("BEAM{}Methods", sanitized_route_str);
            }
        };
        imports_str.push_str(&s);
        (class_name, imports_str, deps)
    }
}

fn mk_params(params: &[intermediate::Param], name: &str) -> (String, String) {
    let mut s_typedef = String::new();
    let mut s_as_json_body = String::new();
    cpf!(s_typedef, "typedef {} = (", name);
    cpf!(s_as_json_body, "final Map<String, String> paramsJson = {{");
    if !params.is_empty() {
        cpf!(s_typedef, "{{");
        for p in params {
            cpf!(
                s_typedef,
                "  /// {}",
                p.description.unwrap_or("").replace("\n", "\n  /// ")
            );
            cpf!(
                s_typedef,
                "  String{} {},",
                if p.required { "" } else { "?" },
                p.name
            );
            cpf!(
                s_as_json_body,
                "  {}'{}': params.{}{},",
                if p.required {
                    String::new()
                } else {
                    format!("if (params.{} != null) ", p.name)
                },
                p.name.replace("$", "\\$"),
                p.name,
                if p.required { "" } else { "!" }
            );
        }
        cpf!(s_typedef, "}}");
    }
    cpf!(s_typedef, ");\n");
    cpf!(s_as_json_body, "}};");
    (s_typedef, s_as_json_body)
}

impl intermediate::Method {
    #[allow(dead_code)]
    fn enum_string(&self) -> String {
        format!("BEAMRequestMethod.{}", self.string())
    }
    fn string(&self) -> &str {
        match self {
            intermediate::Method::Get => "get",
            intermediate::Method::Post => "post",
            intermediate::Method::Put => "put",
            intermediate::Method::Delete => "delete",
            intermediate::Method::Patch => "patch",
            intermediate::Method::Options => "options",
            intermediate::Method::Head => "head",
            intermediate::Method::Trace => "trace",
        }
    }
}
