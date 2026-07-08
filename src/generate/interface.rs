#[derive(Debug, Clone)]
pub struct GenerationArgs {
    pub ignore_deprecated_fields: bool,
    /// Emit immutable/unmodifiable types: `final` properties + `const`
    /// constructors in eager mode, and a private `_rawValue` in lazy mode.
    pub unmodifiable: bool,
    /// Emit map-backed lazy models (properties become getters that parse
    /// `rawValue` on read) instead of eager typed models that parse in
    /// `fromJson`.
    pub lazy: bool,
}

pub trait Generator {
    // generate a list of files (name, content)
    fn generate(
        &self,
        spec: &oas3::Spec,
        args: GenerationArgs,
    ) -> impl std::future::Future<Output = Result<Vec<File>, String>> + Send;
}

pub struct File {
    pub path: std::path::PathBuf,
    pub content: String,
}

/// A chunk of generated Dart source together with the dependent files
/// produced while generating it. Returned by the various `generate_*`
/// helpers in the Dart backend.
pub struct GeneratedCode {
    pub content: String,
    pub files: Vec<File>,
}
