pub struct DartGenerator;
mod readme;
mod serde;

use crate::parse::intermediate;

mod schemes;

impl super::Generator for DartGenerator {
    fn generate(&self, spec: &oas3::Spec) -> Result<Vec<super::File>, String> {
        let class_prefix = "API";
        let class_suffix = "Model";
        let mut out = Vec::new();
        readme::add_readme(&mut out, spec);
        serde::add_serde_utils(&mut out);
        println!("parsing spec to intermediate");
        let intermediate = match intermediate::parse(&spec) {
            Ok(intermediate) => intermediate,
            Err(e) => {
                println!("parsing spec to intermediate error: {:?}", e);
                return Err(format!("parsing spec to intermediate error: {:?}", e));
            }
        };
        let scheme_adder = schemes::SchemeAdder::new(class_prefix, class_suffix);
        scheme_adder.add_schemes(&mut out, &intermediate);
        Ok(out)
    }
}
