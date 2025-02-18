use std::collections::{BTreeMap, HashMap};

pub struct IntermediateFormat<'a> {
    pub schemes: Vec<Scheme<'a>>,
    pub routes: Vec<Route<'a>>,
}

pub struct Scheme<'a> {
    pub name: &'a str,
    pub obj: IAST<'a>,
}

pub enum RouteFragment{
    Node(RouteFragmentNodeData),
    Leaf(RouteFragmentLeafData),
}

pub struct RouteFragmentNodeData{
    pub path_fragment_name : String,
    pub is_param : bool,
}

pub struct RouteFragmentLeafData{
    pub additional_params : Vec<String>
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