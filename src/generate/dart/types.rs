use std::fmt;

pub(super) type BiDt = BuiltinDartType;
#[derive(PartialEq)]
pub(super) enum BuiltinDartType {
    String,
    Number,
    Boolean,
    Integer,
    Object,
    Never,
    List(Box<DartType>),
}

impl fmt::Display for BiDt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            BiDt::String => "String",
            BiDt::Number => "num",
            BiDt::Boolean => "bool", 
            BiDt::Integer => "int",
            BiDt::Object => "Object",
            BiDt::Never => "Never",
            BiDt::List(dt) => return write!(f, "List<{}>", dt),
        };
        write!(f, "{}", s)
    }
}

pub(super) type NnDt = NonNullableDartType;
#[derive(PartialEq)]
pub(super) enum NonNullableDartType {
    Builtin(BiDt),
    Union(Vec<NnDt>),
}

impl fmt::Display for NnDt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NnDt::Builtin(dt) => write!(f, "{}", dt),
            NnDt::Union(dt) => write!(f, "todo"),
        }
    }
}
pub(super) type Dt = DartType;
#[derive(PartialEq)]
pub(super) enum DartType {
    Normal(NnDt),
    Nullable(NnDt),
}

impl fmt::Display for Dt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Dt::Normal(dt) => write!(f, "{}", dt),
            Dt::Nullable(dt) => write!(f, "{}?", dt),
        }
    }
}