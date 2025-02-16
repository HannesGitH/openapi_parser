pub struct DartGenerator;
mod readme;

use crate::parse::intermediate;

mod types;
mod schemes;

impl super::Generator for DartGenerator {
    fn generate(&self, spec: &oas3::Spec) -> Result<Vec<super::File>, String> {
        let class_prefix = "API";
        let class_suffix = "Scheme";
        let mut out = Vec::new();
        readme::add_readme(&mut out, spec);
        println!("parsing spec to intermediate");
        let intermediate = match intermediate::parse(&spec) {
            Ok(intermediate) => intermediate,
            Err(e) => {
                println!("parsing spec to intermediate error: {:?}", e);
                return Err(format!("parsing spec to intermediate error: {:?}", e));
            }
        };
        schemes::SchemeAdder::new(class_prefix, class_suffix).add_schemes(&mut out, &intermediate);
        Ok(out)
    }
}
