
use super::super::interface::*;

use crate::parse::intermediate;

pub(super) struct SchemeAdder<'a> {
    class_prefix: &'a str,
}

impl<'a> SchemeAdder<'a> {
    pub(super) fn new(class_prefix: &'a str) -> Self {
        Self { class_prefix }
    }

    pub(super) fn add_schemes(&self, out: &mut Vec<File>, intermediate: &intermediate::IntermediateFormat) {
        let mut scheme_files = Vec::new();
        for scheme in intermediate.schemes.iter() {
            let (content, depends_on_files) = self.parse_named_iast(scheme.name, &scheme.obj, 0);
            let file = File {
                path: std::path::PathBuf::from(format!("{}.dart", scheme.name)),
                content,
            };
            scheme_files.push(file);
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

    fn parse_named_iast(&self, name: &str, iast: &intermediate::IAST, depth: usize) -> (String, Vec<File>) {
        ("".to_string(), vec![])
    }
}
