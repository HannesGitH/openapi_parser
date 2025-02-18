use std::collections::{BTreeMap, HashMap};

pub struct IntermediateFormat<'a> {
    pub schemes: Vec<Scheme<'a>>,
    pub routes_tree: RouteFragment<'a>,
    pub routes: Vec<Route<'a>>,
}

impl<'a> IntermediateFormat<'a> {
    pub fn new(
        schemes: Vec<Scheme<'a>>, 
        routes: Vec<Route<'a>>, 
        convert_routes_to_tree: for<'b> fn(&'b Vec<Route<'b>>) -> RouteFragment<'b>
    ) -> Self {
        unsafe { 
            let raw_routes = routes.as_ptr() as *const Vec<Route<'a>>;
            let routes_tree = convert_routes_to_tree(&*raw_routes);
            Self { schemes, routes_tree, routes }
        }
    }
}



pub struct Scheme<'a> {
    pub name: &'a str,
    pub obj: IAST<'a>,
}

pub enum RouteFragment<'a> {
    Node(RouteFragmentNodeData<'a>),
    Leaf(RouteFragmentLeafData<'a>),
}

// impl<'a> PartialEq for RouteFragment<'a> {
//     fn eq(&self, other: &Self) -> bool {
//         match (self, other) {
//             (RouteFragment::Node(a), RouteFragment::Node(b)) => a == b,
//             (RouteFragment::Leaf(_), RouteFragment::Leaf(_)) => true,
//             _ => false,
//         }
//     }
// }
pub struct RouteFragmentNodeData<'a> {
    pub path_fragment_name: String,
    pub is_param: bool,
    pub children: Vec<RouteFragment<'a>>,
}

// impl<'a> PartialEq for RouteFragmentNodeData<'a> {
//     fn eq(&self, other: &Self) -> bool {
//         self.path_fragment_name == other.path_fragment_name
//     }
// }

pub struct RouteFragmentLeafData<'a> {
    pub route: &'a Route<'a>,
}

pub struct Route<'a> {
    pub path: &'a str,
    pub description: Option<&'a str>,
    pub endpoints: Vec<Endpoint<'a>>,
}

pub struct Endpoint<'a> {
    pub method: Method,
    pub description: Option<&'a str>,
    pub params: Vec<Param<'a>>,
    pub request: Option<IAST<'a>>,
    pub responses: BTreeMap<&'a String, IAST<'a>>,
}

pub struct Param<'a> {
    pub name: &'a str,
    pub description: Option<&'a str>,
    pub required: bool,
}

pub enum Method {
    Get,
    Post,
    Put,
    Delete,
    Patch,
    Head,
    Options,
    Trace,
}
#[derive(Debug, PartialEq, Eq)]
pub struct AnnotatedObj<'a, T> {
    pub nullable: bool,
    pub is_deprecated: bool,
    pub description: Option<&'a str>,
    pub title: Option<&'a str>,
    pub value: T,
}

/// Intermediate Abstract Syntax Tree
#[derive(Debug, PartialEq, Eq)]
pub enum IAST<'a> {
    Object(AnnotatedObj<'a, AlgType<'a>>),
    /// reference to an ast
    /// ie #/components/schemas/SomeSchema
    Reference(&'a str),
    Primitive(AnnotatedObj<'a, Primitive<'a>>),
}

/// Algebraic Type
#[derive(Debug, PartialEq, Eq)]
pub enum AlgType<'a> {
    Sum(Vec<IAST<'a>>),
    Product(HashMap<&'a str, IAST<'a>>),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Primitive<'a> {
    String,
    Number,
    Integer,
    Boolean,
    Never,
    List(Box<IAST<'a>>),
    Map(Box<IAST<'a>>),
    Enum(Vec<String>),
    Dynamic,
}
