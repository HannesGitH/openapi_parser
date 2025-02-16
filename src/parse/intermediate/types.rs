use std::collections::HashMap;

pub struct IntermediateFormat<'a> {
    pub schemes: Vec<Scheme<'a>>,
}

pub struct Scheme<'a> {
    pub name: &'a str,
    pub obj: IAST<'a>,
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