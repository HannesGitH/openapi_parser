use super::super::interface::*;

pub(super) fn add_readme(out: &mut Vec<File>, spec: &oas3::Spec) {
    out.push(File {
        path: std::path::PathBuf::from("readme.md"),
        content: mk_readme(spec),
    });
}

fn mk_readme(spec: &oas3::Spec) -> String {
    let title = spec.info.title.clone();
    let version = spec.info.version.clone();
    format!(
        "
# {}

this is a dart client for the api described by the oas3 spec.
it is fully generated so *do not edit* it by hand.

## version {}

```dart

```
",
        title, version,
    )
}
