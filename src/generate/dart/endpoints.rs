use crate::{generate::File, parse::intermediate};

pub struct EndpointAdder;

impl EndpointAdder {
    pub fn add_endpoints<'a>(&self, out: &mut Vec<File>, intermediate: &'a intermediate::IntermediateFormat<'a>) {

    }
}