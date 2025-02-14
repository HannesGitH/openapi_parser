pub struct DartGenerator;
mod readme;
mod schemas;
mod types;

impl super::Generator for DartGenerator {
    fn generate(&self, spec: &oas3::Spec) -> Vec<super::File> {
        let class_prefix = "API";
        let mut out = Vec::new();
        readme::add_readme(&mut out, spec);
        schemas::SchemaAdder::new(class_prefix).add_schemas(&mut out, spec);
        out
    }
}
