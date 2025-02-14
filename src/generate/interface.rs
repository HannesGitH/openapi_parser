pub trait Generator {
    // generate a list of files (name, content)
    fn generate(&self, spec: &oas3::Spec) -> Vec<(std::path::PathBuf, String)>;
}
