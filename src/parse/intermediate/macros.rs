use crate::parse::intermediate::*;

pub struct EndpointParser<'a> {
    pub params_parser:
        Box<dyn Fn(&'a Vec<ObjectOrReference<Parameter>>) -> Result<Vec<Param<'a>>, Error>>,
    pub request_parser: Box<dyn Fn(Option<&'a ObjectOrReference<RequestBody>>) -> Result<IAST<'a>, Error>>,
    pub responses_parser: Box<
        dyn Fn(
            &'a BTreeMap<String, ObjectOrReference<Responses>>,
        ) -> Result<BTreeMap<&'a String, IAST<'a>>, Error>,
    >,
}

// this could be a function
#[macro_export]
macro_rules! handle_endpoint {
    ($parser:expr, $endpoints:expr, $route_part:expr, $method:expr) => {{
        if let Some(endpoint) = $route_part {
            $endpoints.push(Endpoint {
                method: $method,
                description: endpoint.description.as_deref(),
                summary: endpoint.summary.as_deref(),
                params: ($parser.params_parser)(&endpoint.parameters).unwrap_or_default(),
                request: match ($parser.request_parser)(endpoint.request_body.as_ref())
                    {
                        Ok(request) => Some(request),
                        Err(e) => {
                            println!("error parsing request: {:?}", e);
                            None
                        }
                    },
                responses: match ($parser.responses_parser)(&endpoint.responses.as_ref().unwrap())
                    {
                        Ok(responses) => responses,
                        Err(e) => {
                            println!("error parsing responses: {:?}", e);
                            BTreeMap::new()
                        }
                    },
            });
        }
    }};
}
