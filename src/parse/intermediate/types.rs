use std::collections::{BTreeMap, HashMap, HashSet};

/// Strip the OpenAPI `$ref` prefix off a path like
/// `#/components/schemas/Foo`, leaving just `Foo`. Idempotent: paths that
/// already lack the prefix are returned unchanged.
///
/// Centralized here so every consumer (parser AND code generators) uses
/// exactly the same notion of "scheme name" — previously this was
/// open-coded as `.replace("#/components/schemas/", "")` in several
/// places, which made it easy to fall out of sync.
pub fn strip_ref_prefix(p: &str) -> &str {
    p.strip_prefix("#/components/schemas/").unwrap_or(p)
}

/// The result of resolving a `$ref` (or an `IAST`) by transitively
/// following references to a concrete top-level scheme.
///
/// The variants describe what kind of Dart value the reference will
/// produce in the generated code — enough information for every codegen
/// site to decide whether the value has `.toJson()` / `.fromJson()`
/// methods, without re-implementing the lookup or the chain-walking
/// logic each time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedRef {
    /// A generated Dart class implementing `BEAMSerde`. Covers all of
    /// `Object`/`Product`/`Sum`/`DiscriminatedSum`. Has
    /// `.toJson()` / `.fromJson()`.
    Class,
    /// A generated Dart enum class. Has `.fromJson()` / `.toJson()`. From a
    /// codegen perspective it behaves like [`Class`](Self::Class) (callers
    /// that only care about "do I call .fromJson?" should treat it the same).
    Enum,
    /// The [`Primitive::Never`] leaf, rendered in Dart as
    /// `UnknownBEAMObject`. It is its own classification — neither a raw
    /// primitive nor a generated class — because `UnknownBEAMObject`
    /// *does* implement `BEAMSerde` (it has `.fromJson()` / `.toJson()`)
    /// and so, for codegen purposes, must be (de)serialized like a
    /// [`Class`](Self::Class) rather than cast like a [`Primitive`](Self::Primitive).
    Never,
    /// A raw Dart primitive value (`String`, `int`, `num`, `bool`,
    /// `dynamic`, `Uint8List`) or a typedef expanding to a
    /// `List<_>` / `Map<String,_>`. These types do *not* have
    /// `.toJson()` / `.fromJson()` methods and must be treated as raw
    /// JSON-compatible values (cast, don't parse).
    Primitive,
    /// We detected a cycle while following references. Treated
    /// conservatively (non-primitive) at the call sites.
    Cycle,
    /// The reference name was not found in the scheme map. Treated
    /// conservatively (non-primitive) at the call sites.
    Unknown,
}

impl ResolvedRef {
    /// True iff this value is a Dart primitive without
    /// `.toJson()` / `.fromJson()` methods, i.e. callers must cast it
    /// from raw JSON rather than calling a generated constructor.
    ///
    /// `Enum` is intentionally NOT primitive: the generator emits a real
    /// Dart enum class with `.fromJson`. `Never` is likewise NOT
    /// primitive: its Dart representation (`UnknownBEAMObject`) has
    /// `.fromJson` / `.toJson` — see [`Self::is_never`].
    pub fn is_primitive(&self) -> bool {
        matches!(self, ResolvedRef::Primitive)
    }

    /// True iff this resolves to the [`Primitive::Never`] leaf
    /// (Dart `UnknownBEAMObject`). Unlike a raw primitive, it must be
    /// (de)serialized via `.fromJson` / `.toJson()` like a class.
    pub fn is_never(&self) -> bool {
        matches!(self, ResolvedRef::Never)
    }
}

pub struct IntermediateFormat<'a> {
    pub schemes: Vec<Scheme<'a>>,
    pub routes_tree: RouteFragment,
    pub routes: Vec<Route<'a>>,
    /// `scheme name -> index into `schemes`. Built once in [`Self::new`]
    /// so look-ups by ref name are O(1) instead of O(n).
    scheme_indices: HashMap<&'a str, usize>,
}

impl<'a> IntermediateFormat<'a> {
    pub fn new(
        schemes: Vec<Scheme<'a>>,
        routes: Vec<Route<'a>>,
        routes_tree: RouteFragment,
    ) -> Self {
        let scheme_indices = schemes
            .iter()
            .enumerate()
            .map(|(i, s)| (s.name, i))
            .collect();
        Self {
            schemes,
            routes_tree,
            routes,
            scheme_indices,
        }
    }

    /// Look up a top-level scheme by its name. The name may be either a
    /// bare scheme name (`Foo`) or a full ref path
    /// (`#/components/schemas/Foo`); both are accepted.
    pub fn find_scheme(&self, name: &str) -> Option<&Scheme<'a>> {
        let stripped = strip_ref_prefix(name);
        self.scheme_indices.get(stripped).map(|&i| &self.schemes[i])
    }

    /// Resolve a `$ref` path (e.g. `#/components/schemas/Foo` or the bare
    /// `Foo`) by transitively following `Reference` chains until a
    /// concrete scheme shape is reached. See [`ResolvedRef`] for the
    /// possible outcomes.
    pub fn resolve_ref(&self, ref_path: &str) -> ResolvedRef {
        let initial = strip_ref_prefix(ref_path);
        let Some(mut current) = self.find_scheme(initial) else {
            return ResolvedRef::Unknown;
        };
        let mut visited: HashSet<&'a str> = HashSet::new();
        visited.insert(current.name);
        loop {
            match &current.obj {
                IAST::Object(_) => return ResolvedRef::Class,
                IAST::Primitive(p) => {
                    return match &p.value {
                        Primitive::Enum(_) => ResolvedRef::Enum,
                        Primitive::Never => ResolvedRef::Never,
                        _ => ResolvedRef::Primitive,
                    };
                }
                IAST::Reference(r) => {
                    let next: &'a str = strip_ref_prefix(r.path);
                    if !visited.insert(next) {
                        return ResolvedRef::Cycle;
                    }
                    current = match self.find_scheme(next) {
                        Some(s) => s,
                        None => return ResolvedRef::Unknown,
                    };
                }
            }
        }
    }

    /// Resolve any [`IAST`] node to a [`ResolvedRef`]. For inline
    /// `Object`/`Primitive` nodes the answer is immediate; for
    /// `Reference` nodes this delegates to [`Self::resolve_ref`] and so
    /// follows reference chains transitively.
    pub fn resolve_iast(&self, iast: &IAST) -> ResolvedRef {
        match iast {
            IAST::Object(_) => ResolvedRef::Class,
            IAST::Primitive(p) => match &p.value {
                Primitive::Enum(_) => ResolvedRef::Enum,
                Primitive::Never => ResolvedRef::Never,
                _ => ResolvedRef::Primitive,
            },
            IAST::Reference(r) => self.resolve_ref(r.path),
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
    pub is_deprecated: bool,
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
    Sum(Vec<SumVariant<'a>>),
    // basically the same as a sum type, but can only have references
    DiscriminatedSum(Discrimination<'a>),
    Product(HashMap<&'a str, IAST<'a>>),
}

/// A single variant of a [`AlgType::Sum`] union: the variant `name`
/// (either the referenced scheme name or the positional index) and the
/// `typ` it resolves to.
#[derive(Debug, PartialEq, Eq)]
pub struct SumVariant<'a> {
    pub name: String,
    pub typ: IAST<'a>,
}

/// A single permitted value of a [`Primitive::Enum`]. `is_string` records
/// whether the value was a JSON string (and so must be emitted quoted in
/// the generated Dart) rather than a native value.
#[derive(Debug, PartialEq, Eq)]
pub struct EnumValue {
    pub value: String,
    pub is_string: bool,
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
    Enum(Vec<EnumValue>),
    Dynamic,
    Binary,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Discrimination<'a> {
    pub key: &'a str,
    pub map: BTreeMap<&'a str, AnnotatedReference<'a>>,
}
