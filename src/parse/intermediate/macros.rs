use crate::parse::intermediate::*;

// type ParamsParser<'a> = Box<dyn Fn(&'a Vec<ObjectOrReference<Parameter>>) -> Result<Vec<Param<'a>>, Error>>;
// type RequestParser<'a> = Box<dyn Fn(&'a ObjectOrReference<RequestBody>) -> Result<IAST<'a>, Error>>;
// type ResponsesParser<'a> = Box<dyn Fn(&'a BTreeMap<&'a str,&ObjectOrReference<Responses>>) -> Result<BTreeMap<&'a str, IAST<'a>>, Error>>;

#[macro_export]
macro_rules! handle_endpoint {
    ($parser:expr, $endpoints:expr, $route_part:expr, $method:expr, $params_parser:expr, $request_parser:expr, $responses_parser:expr) => {
        {
            if let Some(endpoint) = $route_part {
                $endpoints.push(Endpoint {
                    method: $method,
                    description: endpoint.description.as_deref(),
                    params: $params_parser(&endpoint.parameters).ok(),
                    request: $request_parser(&endpoint.request_body.as_ref().unwrap()).unwrap(),
                    responses: $responses_parser(&endpoint.responses.as_ref().unwrap()).unwrap(),
                });
            }
        }
    };
}
