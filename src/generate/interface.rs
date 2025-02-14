pub trait Generator {
    // generate a list of files (name, content)
    fn generate(&self, spec: &oas3::Spec) -> Vec<File>;
}

pub struct File {
    pub path: std::path::PathBuf,
    pub content: String,
}
