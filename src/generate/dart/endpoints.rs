use crate::{
    generate::File,
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

        let mut imports_content = String::new();
        imports_content.push_str(&include_str!("endpoints/imports.dart"));

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
        cpf!(c, "import '{}endpoints.dart';", "../".repeat(depth));
        cpf!(c, "/// {}", route.path);
        cpf!(c, "/// {}",route.description.unwrap_or("").replace("\n", "\n/// "));
        cpf!(c, "class API{} extends APIPath {{", name);
        cpf!(c, "  API{}({{required super.interpolator, required super.handler}})", name);
        cpf!(c, "    :super(path: APIPathEnum.fromJson('{}'));", route.path);
        for method in &route.endpoints {
            let method_str = method.method.string();
            let param_name = format!("_P_{}", method_str);
            let body_str = match &method.request {
                Some(request) => {
                    //do things
                    let request_name = format!("{}Request", name);
                    let (content, sub_deps, not_built) = self.scheme_adder.parse_named_iast(&request_name, request, depth+1);
                    deps.extend(sub_deps.into_iter().map(|f| File {
                        path: std::path::PathBuf::from(format!("{}/{}", name, f.path.to_str().unwrap())),
                        content: f.content,
                    }));
                    let dep_path_str = format!("{}/{}.body.schema.dart", name, method_str);
                    let dep_path = std::path::PathBuf::from(&dep_path_str);
                    deps.push(File {
                        path: dep_path,
                        content,
                    });
                    imports_str.push_str(&format!("import '{}';\n", &dep_path_str));
                    let body_type_str = match not_built {
                        Some(schemes::NotBuiltData {reason: _, type_name}) => type_name,
                        _ => self.scheme_adder.class_name(&request_name),
                    };
                    format!(" {{required {} body}}", body_type_str)
                },
                None => String::new(),
            };
            let params_str = if method.params.is_empty() { String::new() } else { format!("{} params,", param_name) };
            //TODO
            let ret_type_str = String::new();
            //TODO
            let impl_str = String::new();

            param_typedef_strs.push_str(&mk_params(&method.params, &param_name));
            cpf!(c, "  {} {}({}{}){{{}}}", ret_type_str, method_str, params_str, body_str, impl_str);
        }
        cpf!(c, "}}\n");
        c.push_str(&param_typedef_strs);
        imports_str.push_str(&c);
        (imports_str, deps)
    }
}

fn mk_params(params: &[intermediate::Param], name: &str) -> String {
    let mut s = String::new();
    s.push_str(&format!("typedef {} = (", name));
    if !params.is_empty() {
        cpf!(s, "{{");
        for p in params {
            cpf!(s, "  /// {}", p.description.unwrap_or("").replace("\n", "\n  /// "));
            cpf!(s, "  String{} {},", if p.required { "" } else { "?" }, p.name);
        }
        cpf!(s, "}}");
    }
    cpf!(s, ");\n");
    s
}

impl intermediate::Method {
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
