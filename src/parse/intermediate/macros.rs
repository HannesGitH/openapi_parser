use crate::parse::intermediate::*;

pub struct EndpointParser<'a> {
    pub params_parser:
        Box<dyn Fn(&'a Vec<ObjectOrReference<Parameter>>) -> Result<Vec<Param<'a>>, Error>>,
    pub request_parser: Box<dyn Fn(&'a ObjectOrReference<RequestBody>) -> Result<IAST<'a>, Error>>,
    pub responses_parser: Box<
        dyn Fn(
            &'a BTreeMap<String, ObjectOrReference<Responses>>,
        ) -> Result<BTreeMap<&'a String, IAST<'a>>, Error>,
    >,
}

#[macro_export]
macro_rules! handle_endpoint {
    ($parser:expr, $endpoints:expr, $route_part:expr, $method:expr) => {{
        if let Some(endpoint) = $route_part {
            $endpoints.push(Endpoint {
                method: $method,
                description: endpoint.description.as_deref(),
                params: ($parser.params_parser)(&endpoint.parameters).ok(),
                request: ($parser.request_parser)(&endpoint.request_body.as_ref().unwrap())
                    .unwrap(),
                responses: ($parser.responses_parser)(&endpoint.responses.as_ref().unwrap())
                    .unwrap(),
            });
        }
    }};
}
