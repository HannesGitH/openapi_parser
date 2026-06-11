use crate::{
    generate::{
        dart::schemes::{
            create_property_name, sanitize, sanitize_identifier, GenerationSpecialCaseType,
        },
        File, GeneratedCode,
    },
    parse::intermediate::{self, Route, RouteFragmentLeafData},
};

use super::schemes;

/// Result of [`EndpointAdder::generate_route_fragment`]: the generated
/// fragment class, its own class name (so the parent can reference it)
/// and the files it depends on.
struct GeneratedFragment {
    class_name: String,
    content: String,
    files: Vec<File>,
}

/// Result of [`mk_params`]: the Dart `typedef` for the params record and
/// the snippet that builds the `paramsJson` map from those params.
struct ParamsCode {
    typedef: String,
    as_json_body: String,
}

/// Classification of a request body, driving how it is serialized before
/// being sent. `decl` is the Dart parameter declaration (e.g.
/// ` {required Foo body}`) or `None` when the endpoint takes no body.
struct BodyClass {
    decl: Option<String>,
    is_primitive: bool,
    list_inner_type: Option<String>,
}

/// Classification of a response value, driving how it is decoded.
struct ResponseClass {
    type_str: String,
    is_primitive: bool,
    list_inner_type: Option<String>,
    is_binary: bool,
}

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
        let paths_enum = self.scheme_adder.generate_primitive_sum_type(
            "Paths",
            "",
            &intermediate
                .routes
                .iter()
                // we always have a string path, so we can always use true here (i guess)
                .map(|r| schemes::AllowedValue {
                    value: r.path,
                    is_string: true,
                    description: r.description.unwrap_or(""),
                })
                .collect::<Vec<_>>(),
        );

        for route in &intermediate.routes {
            let sanitized_path = sanitize(route.path);
            let name = format!("{}Methods", sanitized_path);
            let wrapper = self.generate_path_method_wrapper(&name, route, 1);
            out_files.push(File {
                path: std::path::PathBuf::from(format!("routes/{}.dart", sanitized_path)),
                content: wrapper.content,
            });
            out_files.extend(wrapper.files.into_iter().map(|f| File {
                path: std::path::PathBuf::from(format!("routes/{}", f.path.to_str().unwrap())),
                content: f.content,
            }));
        }

        let frag = self.generate_route_fragment(
            "root",
            &intermediate.routes_tree,
            &intermediate.routes,
            true,
            0,
        );
        let root_frag_file_name = "root_fragment.dart";
        out_files.push(File {
            path: std::path::PathBuf::from(root_frag_file_name),
            content: frag.content,
        });
        out_files.extend(frag.files);

        let mut imports_content = String::new();
        imports_content.push_str(&include_str!("endpoints/imports.dart"));
        cpf!(imports_content, "import '{}';", root_frag_file_name);
        let mut content = String::new();
        content.push_str(&imports_content);
        content.push_str(&format!(
            "typedef BEAMPathEnum={};\n",
            paths_enum.class_name
        ));
        content.push_str(&paths_enum.content);
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

    /// Parse a single response IAST, emit its schema file + import/export
    /// lines (appended to `deps`/`imports_str`), and classify it into a
    /// [`ResponseClass`] describing how the value should be decoded.
    ///
    /// Extracted so a future multi-response parser can call it per status
    /// code and combine the results into a response union type.
    fn process_response(
        &self,
        response_name: &str,
        response_code: &str,
        response: &intermediate::IAST,
        name: &str,
        method_str: &str,
        depth: usize,
        deps: &mut Vec<File>,
        imports_str: &mut String,
    ) -> ResponseClass {
        let parsed = self
            .scheme_adder
            .parse_named_iast(response_name, response, depth + 1);
        let dep_path_str = format!("{}/{}.resp.{}.schema.dart", name, method_str, response_code);
        let dep_path = std::path::PathBuf::from(&dep_path_str);
        deps.push(File {
            path: dep_path,
            content: parsed.content,
        });
        deps.extend(parsed.files.into_iter().map(|f| File {
            path: std::path::PathBuf::from(format!("{}/{}", name, f.path.to_str().unwrap())),
            content: f.content,
        }));
        imports_str.push_str(&format!("import '{}';\n", &dep_path_str));
        imports_str.push_str(&format!("export '{}';\n", &dep_path_str));
        match parsed.special_case {
            Some(schemes::GenerationSpecialCase { reason, type_name }) => ResponseClass {
                type_str: type_name,
                // Is the response value a raw primitive
                // (no `.fromJson`) rather than a generated
                // class? Three sub-cases:
                //  - the IAST itself was primitive,
                //  - it was a list whose elements were
                //    flagged primitive,
                //  - it was a link (`$ref`) whose target
                //    transitively resolves to a primitive
                //    typedef (e.g. `typedef Foo = String;`).
                // The link case used to be a one-shot
                // open-coded `iter().find` only one hop
                // deep; it now goes through the shared
                // resolver which follows ref chains and
                // correctly classifies enums as non-primitive.
                is_primitive: match &reason {
                    GenerationSpecialCaseType::Primitive => true,
                    GenerationSpecialCaseType::List(_, is_primitive) => *is_primitive,
                    GenerationSpecialCaseType::Link(link) => {
                        self.intermediate.resolve_ref(link).is_primitive()
                    }
                },
                list_inner_type: if let GenerationSpecialCaseType::List(inner_type, _) = reason {
                    Some(inner_type)
                } else {
                    None
                },
                is_binary: parsed.is_binary,
            },
            None => ResponseClass {
                type_str: self.scheme_adder.class_name(response_name),
                is_primitive: false,
                list_inner_type: None,
                is_binary: parsed.is_binary,
            },
        }
    }

    fn generate_path_method_wrapper(
        &self,
        name: &str,
        route: &Route,
        depth: usize,
    ) -> GeneratedCode {
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
        cpf!(imports_str, "import 'dart:typed_data';\n");
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
            "    :super(path: BEAMPathEnum.t_{});",
            sanitize(route.path)
        );
        for method in &route.endpoints {
            let method_str = method.method.string();
            let param_name = format!("_P_{}", method_str);
            let body_class = match &method.request {
                Some(request) => {
                    //do things
                    let request_name = format!("{}{}Request", name, method_str);
                    let parsed =
                        self.scheme_adder
                            .parse_named_iast(&request_name, request, depth + 1);
                    deps.extend(parsed.files.into_iter().map(|f| File {
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
                        content: parsed.content,
                    });
                    imports_str.push_str(&format!("import '{}';\n", &dep_path_str));
                    imports_str.push_str(&format!("export '{}';\n", &dep_path_str));
                    // Classify the request body so the impl emission below
                    // can pick the right "serialize before sending" form.
                    // Three buckets:
                    //   * primitive (no .toJson)  -> send `body` raw
                    //   * single object/enum     -> send `body?.toJson()`
                    //   * list of object/enum    -> send `body?.map((e) => e.toJson()).toList()`
                    // For lists, we rely on the same `is_primitive` flag the
                    // response side uses (set by `parse_named_iast` and now
                    // chain-following via the shared `resolve_ref`), so
                    // `List<Primitive>` and `List<$ref to primitive typedef>`
                    // both bucket as primitive.
                    match parsed.special_case {
                        Some(schemes::GenerationSpecialCase { reason, type_name }) => BodyClass {
                            decl: Some(format!(" {{required {} body}}", type_name)),
                            is_primitive: match &reason {
                                GenerationSpecialCaseType::List(_, is_primitive) => *is_primitive,
                                GenerationSpecialCaseType::Primitive => true,
                                // A link can point at a primitive
                                // typedef (e.g. `typedef Foo = String;`);
                                // in that case the body has no
                                // `.toJson()` and must be passed as raw
                                // JSON. Defer to the shared resolver so
                                // chains of refs are followed too.
                                GenerationSpecialCaseType::Link(link) => {
                                    self.intermediate.resolve_ref(link).is_primitive()
                                }
                            },
                            list_inner_type: match reason {
                                GenerationSpecialCaseType::List(inner_type, _) => Some(inner_type),
                                _ => None,
                            },
                        },
                        _ => BodyClass {
                            decl: Some(format!(
                                " {{required {} body}}",
                                self.scheme_adder.class_name(&request_name)
                            )),
                            is_primitive: false,
                            list_inner_type: None,
                        },
                    }
                }
                None => BodyClass {
                    decl: None,
                    is_primitive: true,
                    list_inner_type: None,
                },
            };
            let params_str = if method.params.is_empty() {
                String::new()
            } else {
                format!("{} params,", param_name)
            };
            let params = mk_params(&method.params, &param_name);

            let response_class = {
                let responses = &method.responses;
                if responses.is_empty() {
                    ResponseClass {
                        type_str: "()".to_string(),
                        is_primitive: true,
                        list_inner_type: None,
                        is_binary: false,
                    }
                } else if responses.len() == 1 {
                    let response_name = format!("{}_{}Response", name, method_str);
                    let (response_code, response) = responses.first_key_value().unwrap();
                    self.process_response(
                        &response_name,
                        response_code,
                        response,
                        name,
                        &method_str,
                        depth,
                        &mut deps,
                        &mut imports_str,
                    )
                } else {
                    //TODO: add parser for multiple responses
                    let response_name = format!("{}_{}Response", name, method_str);
                    let (response_code, response) = responses.first_key_value().unwrap();
                    self.process_response(
                        &response_name,
                        response_code,
                        response,
                        name,
                        &method_str,
                        depth,
                        &mut deps,
                        &mut imports_str,
                    )
                }
            };
            let impl_str = {
                let mut s = String::new();
                s.push_str(&format!(
                    "\n\t\t{}",
                    params.as_json_body.replace("\n", "\n\t\t")
                ));
                let body_emission: String = match (
                    &body_class.decl,
                    body_class.is_primitive,
                    &body_class.list_inner_type,
                ) {
                    // No body: send `null`.
                    (None, _, _) => "null".to_string(),
                    // Primitive body (single primitive OR `List<Primitive>`):
                    // send raw, no serialization step needed.
                    (Some(_), true, _) => "body".to_string(),
                    // Non-primitive list body (`List<Object>` /
                    // `List<Enum>`): Dart's built-in `List` has no
                    // `.toJson()`, so we must serialize element-by-element.
                    (Some(_), false, Some(_)) => {
                        "body?.map((e) => e.toJson()).toList()".to_string()
                    }
                    // Single non-primitive body: call .toJson() directly.
                    (Some(_), false, None) => "body?.toJson()".to_string(),
                };
                cpf!(s, "return handleCached(method: BEAMRequestMethod.{}, params: paramsJson, body: {}, expectedResponseType: {}).then((json) => {});", method_str, body_emission, match response_class.is_binary {
                    true => "BEAMExpectedResponseType.binary",
                    false => "BEAMExpectedResponseType.json",
                }, match (response_class.is_primitive, &response_class.list_inner_type) {
                    (true, None) => "json".to_string(),
                    (true, Some(_)) => format!("json"),
                    (false, Some(inner_type)) => format!("(json as List).map((e) => {}.fromJson(e)).toList()", inner_type),
                    (false, None) => format!("{}.fromJson(json)", response_class.type_str),
                });
                s
            };

            param_typedef_strs.push_str(&params.typedef);
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
                "  BEAMCachedResponse<{}> {}({}{}){{{}\t}}",
                response_class.type_str,
                method_str,
                params_str,
                match &body_class.decl {
                    Some(decl) => decl,
                    None => "",
                },
                impl_str
            );
        }
        cpf!(c, "}}\n");
        c.push_str(&param_typedef_strs);
        // c.push_str(&all_ret_types_str);
        imports_str.push_str(&c);
        GeneratedCode {
            content: imports_str,
            files: deps,
        }
    }

    /// returns its own class name, the content to build it and the files it depends on
    fn generate_route_fragment(
        &self,
        name: &str,
        fragment: &intermediate::RouteFragment,
        routes: &[Route],
        is_root: bool,
        depth: usize,
    ) -> GeneratedFragment {
        let name = sanitize(name);
        let mut s = String::new();
        let mut deps = Vec::new();
        let mut imports_str = String::new();

        use intermediate::RouteFragment;
        let class_name;
        match fragment {
            RouteFragment::Node(node) => {
                cpf!(
                    imports_str,
                    "import '{}endpoints.dart';",
                    "../".repeat(depth)
                );
                let sanitized_frag_name = sanitize(node.path_fragment_name.as_str());
                class_name = format!("BEAM{}Frag_{}", name, sanitized_frag_name);

                let sub_dir_name = format!("{}_frags", sanitized_frag_name);
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
                    let child_frag =
                        self.generate_route_fragment(&child_name, child, routes, false, depth + 1);
                    let child_file_name = format!(
                        "{}/{}.dart",
                        sub_dir_name,
                        match child {
                            intermediate::RouteFragment::Node(child_node) => {
                                let child_frag_name = &child_node.path_fragment_name;
                                // The fragment name is also used as a Dart
                                // method/getter name here, so it must be a
                                // valid identifier (no leading digit).
                                let child_getter_name = sanitize_identifier(child_frag_name);
                                if child_node.is_param {
                                    cpf!(
                                        s,
                                        "\t{} {}(String param) => {}(parent: this, param: param, deps: this.deps);",
                                        child_frag.class_name,
                                        child_getter_name,
                                        child_frag.class_name
                                    );
                                } else {
                                    cpf!(
                                        s,
                                        "\t{} get {} => {}(parent: this, deps: this.deps);",
                                        child_frag.class_name,
                                        child_getter_name,
                                        child_frag.class_name
                                    );
                                };
                                child_frag_name
                            }
                            intermediate::RouteFragment::Leaf(_) => {
                                cpf!(
                                    s,
                                    "\t{} call() => {}(interpolatedPath: this.path, handler: this.deps);",
                                    child_frag.class_name,
                                    child_frag.class_name
                                );
                                "leaf"
                            }
                        }
                    );
                    let sub_frag_file = File {
                        path: std::path::PathBuf::from(&child_file_name),
                        content: child_frag.content,
                    };
                    deps.extend(child_frag.files.into_iter().map(|f| File {
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
                let route = &(routes[*route_idx]);
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
        GeneratedFragment {
            class_name,
            content: imports_str,
            files: deps,
        }
    }
}

fn mk_params(params: &[intermediate::Param], name: &str) -> ParamsCode {
    let mut s_typedef = String::new();
    let mut s_as_json_body = String::new();
    cpf!(s_typedef, "typedef {} = (", name);
    cpf!(s_as_json_body, "final Map<String, String> paramsJson = {{");
    if !params.is_empty() {
        cpf!(s_typedef, "{{");
        for p in params {
            let p_ident = create_property_name(p.name);
            cpf!(
                s_typedef,
                "  /// {}",
                p.description.unwrap_or("").replace("\n", "\n  /// ")
            );
            cpf!(
                s_typedef,
                "  String{} {},",
                if p.required { "" } else { "?" },
                p_ident
            );
            cpf!(
                s_as_json_body,
                "  {}'{}': params.{}{},",
                if p.required {
                    String::new()
                } else {
                    format!("if (params.{} != null) ", p_ident)
                },
                p.name.replace("$", "\\$"),
                p_ident,
                if p.required { "" } else { "!" }
            );
        }
        cpf!(s_typedef, "}}");
    }
    cpf!(s_typedef, ");\n");
    cpf!(s_as_json_body, "}};");
    ParamsCode {
        typedef: s_typedef,
        as_json_body: s_as_json_body,
    }
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
