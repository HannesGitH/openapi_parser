use std::collections::{BTreeMap, HashMap};

pub struct IntermediateFormat<'a> {
    pub schemes: Vec<Scheme<'a>>,
    pub routes_tree: RouteFragment,
    pub routes: Vec<Route<'a>>,
}

impl<'a> IntermediateFormat<'a> {
    pub fn new(
        schemes: Vec<Scheme<'a>>,
        routes: Vec<Route<'a>>,
        routes_tree: RouteFragment,
    ) -> Self {
        Self {
            schemes,
            routes_tree,
            routes,
        }
    }
}

pub struct Scheme<'a> {
    pub name: &'a str,
    pub is_inherently_nullable: bool,
    pub obj: IAST<'a>,
}

pub enum RouteFragment {
    Node(RouteFragmentNodeData),
    Leaf(RouteFragmentLeafData),
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
pub struct RouteFragmentNodeData {
    pub path_fragment_name: String,
    pub is_param: bool,
    pub children: Vec<RouteFragment>,
}

// impl<'a> PartialEq for RouteFragmentNodeData<'a> {
//     fn eq(&self, other: &Self) -> bool {
//         self.path_fragment_name == other.path_fragment_name
//     }
// }

pub struct RouteFragmentLeafData {
    pub route_idx: usize,
}

pub struct Route<'a> {
    pub path: &'a str,
    pub description: Option<&'a str>,
    pub endpoints: Vec<Endpoint<'a>>,
}

pub struct Endpoint<'a> {
    pub method: Method,
    pub description: Option<&'a str>,
    pub summary: Option<&'a str>,
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
    pub optional: bool,
    pub is_deprecated: bool,
    pub description: Option<&'a str>,
    pub title: Option<&'a str>,
    pub value: T,
}

#[derive(Debug, PartialEq, Eq)]
pub struct AnnotatedReference<'a> {
    pub path: &'a str,
    pub optional: bool,
    pub nullable: bool,
}

/// Intermediate Abstract Syntax Tree
#[derive(Debug, PartialEq, Eq)]
pub enum IAST<'a> {
    Object(AnnotatedObj<'a, AlgType<'a>>),
    /// reference to an ast
    /// ie #/components/schemas/SomeSchema
    Reference(AnnotatedReference<'a>),
    Primitive(AnnotatedObj<'a, Primitive<'a>>),
}

/// Algebraic Type
#[derive(Debug, PartialEq, Eq)]
pub enum AlgType<'a> {
    // name -> type
    Sum(Vec<(String, IAST<'a>)>),
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
    // each (value, is_string), is_string
    Enum(Vec<(String, bool)>),
    Dynamic,
}
