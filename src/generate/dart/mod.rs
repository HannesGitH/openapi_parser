pub struct DartGenerator;

impl super::Generator for DartGenerator {
    fn generate(&self, spec: &oas3::Spec) -> Vec<(std::path::PathBuf, String)> {
        vec![]
    }
}