#[derive(Debug, Clone)]
pub struct GenerationArgs {
    pub ignore_deprecated_fields: bool,
}

pub trait Generator {
    // generate a list of files (name, content)
    async fn generate(&self, spec: &oas3::Spec, args: GenerationArgs) -> Result<Vec<File>, String>;
}

pub struct File {
    pub path: std::path::PathBuf,
    pub content: String,
}
