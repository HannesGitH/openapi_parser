use crate::{
    generate::{dart::schemes::NotBuiltReason, File},
    parse::intermediate::{self, Route},
};

use super::schemes;

#[macro_use]
mod macros;
pub struct EndpointAdder<'a> {
    scheme_adder: &'a schemes::SchemeAdder<'a>,
}

impl<'a> EndpointAdder<'a> {
    pub fn new(scheme_adder: &'a schemes::SchemeAdder<'a>) -> Self {
        Self { scheme_adder }
    }
    pub fn add_endpoints(
        &self,
        out: &mut Vec<File>,
        intermediate: &'a intermediate::IntermediateFormat<'a>,
    ) {
        let mut out_files: Vec<File> = Vec::new();
        let interface_content = include_str!("endpoints/interface.dart");
        let (path_enum_name, paths_enum_content) = self.scheme_adder.generate_primitive_sum_type(
            "Paths",
            "",
            &intermediate
                .routes
                .iter()
                .map(|r| (r.path, r.description.unwrap_or("")))
                .collect::<Vec<(&str, &str)>>(),
        );

        for route in &intermediate.routes {
            let sanitized_path = route
                .path
                .chars()
                .map(|c| if c.is_alphanumeric() { c } else { '_' })
                .collect::<String>();
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

        let (frag_class_name, frag_content, frag_deps) =
            self.generate_route_fragment("root", &intermediate.routes_tree, 0);
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
        content.push_str(&format!("typedef APIPathEnum={};\n", path_enum_name));
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
        let mut all_ret_types_str = String::new();
        cpf!(c, "import '{}endpoints.dart';", "../".repeat(depth));
        cpf!(c, "/// {}", route.path);
        cpf!(
            c,
            "/// {}",
            route.description.unwrap_or("").replace("\n", "\n/// ")
        );
        cpf!(c, "class API{} extends APIPath {{", name);
        cpf!(
            c,
            "  API{}({{required super.interpolator, required super.handler}})",
            name
        );
        cpf!(
            c,
            "    :super(path: APIPathEnum.fromJson('{}'));",
            route.path
        );
        for method in &route.endpoints {
            let method_str = method.method.string();
            let param_name = format!("_P_{}", method_str);
            let (body_str, body_is_primitive) = match &method.request {
                Some(request) => {
                    //do things
                    let request_name = format!("{}{}Request", name, method_str);
                    let (content, sub_deps, not_built) =
                        self.scheme_adder
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
                    let body_type_str = match not_built {
                        Some(schemes::NotBuiltData { reason, type_name }) => {
                            (type_name, reason == NotBuiltReason::Primitive)
                        }
                        _ => (self.scheme_adder.class_name(&request_name), false),
                    };
                    (
                        Some(format!(" {{required {} body}}", body_type_str.0)),
                        body_type_str.1,
                    )
                }
                None => (None, true),
            };
            let params_str = if method.params.is_empty() {
                String::new()
            } else {
                format!("{} params,", param_name)
            };
            let (params_1typedef_str, params_as_json_body_str) =
                &mk_params(&method.params, &param_name);

            let (ret_type_str, ret_is_primitive) = {
                let responses = &method.responses;
                if responses.is_empty() {
                    ("()".to_string(), true)
                } else {
                    let response_name = format!("{}_{}Response", name, method_str);
                    if responses.len() == 1 {
                        let (response_code, response) = responses.first_key_value().unwrap();
                        let (content, sub_deps, not_built) = self.scheme_adder.parse_named_iast(
                            &response_name,
                            &response,
                            depth + 1,
                        );
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
                        match not_built {
                            Some(schemes::NotBuiltData {
                                reason: _,
                                type_name,
                            }) => (type_name, true),
                            _ => (self.scheme_adder.class_name(&response_name), false),
                        }
                    } else {
                        //TODO
                        ("TODO".to_string(), false)
                    }
                }
            };
            let impl_str = {
                let mut s = String::new();
                s.push_str(&format!(
                    "\n\t\t{}",
                    params_as_json_body_str.replace("\n", "\n\t\t")
                ));
                cpf!(s, "\t\treturn handle(method: APIRequestMethod.{}, params: paramsJson, body: {}).then((json) => {});", method_str,match (&body_str, &body_is_primitive) {
                    (Some(_), true) => "body",
                    (Some(_), false) => "body.toJson()",
                    (None, _) => "null",
                }, if ret_is_primitive { "json".to_string() } else { format!("{}.fromJson(json)", ret_type_str) });
                s
            };

            param_typedef_strs.push_str(&params_1typedef_str);
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
        c.push_str(&all_ret_types_str);
        imports_str.push_str(&c);
        (imports_str, deps)
    }

    /// returns its own class name, the content to build it and the files it depends on
    fn generate_route_fragment(
        &self,
        name: &str,
        fragment: &intermediate::RouteFragment,
        depth: usize,
    ) -> (String, String, Vec<File>) {
        let mut s = String::new();
        let mut deps = Vec::new();
        let mut imports_str = String::new();
        cpf!(
            imports_str,
            "import '{}endpoints.dart';",
            "../".repeat(depth)
        );
        use intermediate::RouteFragment;
        let mut class_name = format!("API{}Frag_{}", name, "unknown");
        match fragment {
            RouteFragment::Node(node) => {
                class_name = format!("API{}Frag_{}", name, node.path_fragment_name);
                let sub_dir_name = format!("{}_frags", node.path_fragment_name);
                cpf!(s, "class {} extends APIWithParent {{", class_name);
                cpf!(
                    s,
                    "\t{}({{required super.parent}}) : super(ownFragment: '{}');",
                    class_name,
                    node.path_fragment_name
                );
                for child in &node.children {
                    let child_name = format!("{}_{}", name, node.path_fragment_name);
                    let (child_class_name, child_str, child_deps) =
                        self.generate_route_fragment(&child_name, child, depth + 1);
                    let child_file_name = format!(
                        "{}/{}.dart",
                        sub_dir_name,
                        match child {
                            intermediate::RouteFragment::Node(child_node) => {
                                &child_node.path_fragment_name
                            }
                            intermediate::RouteFragment::Leaf(_) => {
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
                }
                cpf!(s, "}}\n");
            }
            RouteFragment::Leaf(leaf) => {
                s.push_str(&format!("//TODO: {}", leaf.route_idx));
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
                p.name,
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
        format!("APIRequestMethod.{}", self.string())
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
