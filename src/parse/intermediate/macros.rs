use crate::parse::intermediate::*;

/// `'p` is the lifetime of the borrows held by the parser itself (function
/// pointers and the ctx reference). `'a` is the lifetime of the OpenAPI spec
/// data that the produced IAST borrows from. They are intentionally distinct:
/// the parser is a short-lived local value while the spec data outlives it.
pub struct EndpointParser<'p, 'a> {
    pub ctx: &'p ParseCtx<'a>,
    pub params_parser: &'p dyn for<'b> Fn(
        &'b ParseCtx<'a>,
        &'a Vec<ObjectOrReference<Parameter>>,
    ) -> Result<Vec<Param<'a>>, Error>,
    pub request_parser: &'p dyn for<'b> Fn(
        &'b ParseCtx<'a>,
        Option<&'a ObjectOrReference<RequestBody>>,
    ) -> Result<IAST<'a>, Error>,
    pub responses_parser: &'p dyn for<'b> Fn(
        &'b ParseCtx<'a>,
        &'a BTreeMap<String, ObjectOrReference<Responses>>,
    ) -> Result<BTreeMap<&'a String, IAST<'a>>, Error>,
}

// this could be a function
#[macro_export]
macro_rules! handle_endpoint {
    ($parser:expr, $endpoints:expr, $route_part:expr, $method:expr) => {{
        if let Some(endpoint) = $route_part {
            // Skip whole operations marked deprecated when the flag is on.
            let skip_deprecated = $parser.ctx.args.ignore_deprecated_fields
                && endpoint.deprecated.unwrap_or(false);
            if !skip_deprecated {
                $endpoints.push(Endpoint {
                    method: $method,
                    description: endpoint.description.as_deref(),
                    summary: endpoint.summary.as_deref(),
                    params: ($parser.params_parser)($parser.ctx, &endpoint.parameters)
                        .unwrap_or_default(),
                    request: match ($parser.request_parser)(
                        $parser.ctx,
                        endpoint.request_body.as_ref(),
                    ) {
                        Ok(request) => Some(request),
                        Err(e) => {
                            println!("error parsing request: {:?}", e);
                            None
                        }
                    },
                    responses: match ($parser.responses_parser)(
                        $parser.ctx,
                        &endpoint.responses.as_ref().unwrap(),
                    ) {
                        Ok(responses) => responses,
                        Err(e) => {
                            println!("error parsing responses: {:?}", e);
                            BTreeMap::new()
                        }
                    },
                });
            }
        }
    }};
}
