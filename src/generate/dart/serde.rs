use super::super::interface::*;

pub(super) fn add_serde_utils(out: &mut Vec<File>) {
    out.push(File {
        path: std::path::PathBuf::from("utils/serde.dart"),
        content: mk_serde_utils(),
    });
}

fn mk_serde_utils() -> String {
    include_str!("serde.dart").to_string()
}
