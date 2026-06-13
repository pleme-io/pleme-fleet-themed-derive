//! `#[derive(FleetThemed)]` — mechanize the `ishou_tokens::FleetThemedConfig`
//! `from_fleet(&FleetDefaults) -> Self` constructor.
//!
//! # Why
//!
//! Every fleet app with visual config (mado, namimado, escriba, hibiki,
//! hikki, …) hand-writes `impl FleetThemedConfig::from_fleet`, a flat
//! body of `field: fd.<field>.clone()` assignments plus a small
//! app-specific tail (nested theme-surface mapping, per-OS decoration
//! splits, value transforms). The fleet audit found ONE production impl
//! (mado@556b685) — every other app hand-pins constants and drifts.
//!
//! `FleetThemed` turns the mechanizable common part — the flat
//! `FleetDefaults → field` assignments — into one derive line, leaving
//! the app a small typed escape hatch (`with = …`, `finalize = …`) for
//! the genuinely-unique tail. Per the pleme-io ★★ EMITTER SUBSTRATE
//! rule: every recurring impl is a Spec.
//!
//! # Usage
//!
//! ```rust,ignore
//! use pleme_fleet_themed_derive::FleetThemed;
//!
//! #[derive(FleetThemed)]
//! #[fleet(base = "bare_base")] // optional; default = Default::default
//! struct MyAppConfig {
//!     // by-name: field name matches a FleetDefaults field → fd.theme
//!     #[fleet]                  theme: String,
//!     // explicit source field name
//!     #[fleet(font_family)]     family: String,
//!     // Copy fleet field (no .clone())
//!     #[fleet(font_size)]       size: f32,
//!     // transform via a fn(&FleetDefaults) -> T escape hatch
//!     #[fleet(with = map_cursor)] cursor: CursorStyle,
//!     // literal default, no fleet analogue
//!     #[fleet(default = 80)]    cols: u32,
//!     // pull from the base config (skip / unannotated both do this)
//!     #[fleet(skip)]            profiles: HashMap<String, Profile>,
//!     shell: ShellConfig,       // unannotated == skip
//! }
//! ```
//!
//! Field attribute grammar (`#[fleet(...)]`):
//!
//! | form                       | emits                                  |
//! |----------------------------|----------------------------------------|
//! | `#[fleet]`                 | `field: fd.field.clone()`              |
//! | `#[fleet(<src>)]`          | `field: fd.<src>.clone()`              |
//! | `#[fleet(<src>, copy)]`    | `field: fd.<src>` (no clone)           |
//! | `#[fleet(with = path)]`    | `field: path(fd)`                      |
//! | `#[fleet(default = expr)]` | `field: expr`                          |
//! | `#[fleet(skip)]` / none    | (taken from the base — struct-update)  |
//!
//! Struct attribute grammar (`#[fleet(...)]` on the item):
//!
//! | form                        | effect                                 |
//! |-----------------------------|----------------------------------------|
//! | `#[fleet(base = "path")]`   | base ctor for skipped fields (default `Default::default`) |
//! | `#[fleet(finalize = path)]` | call `path(&mut self, fd)` after build |
//! | `#[fleet(copy_default)]`    | scalar fleet fields copied by default (no `.clone()`) |
//!
//! The crate emits source through `quote!{}` (★★ TYPED EMISSION); no
//! `std::format!`. Token-stability snapshot tests assert on the emitted
//! token stream, never on prettyplease output.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{
    Data, DataStruct, DeriveInput, Expr, Fields, Ident, Path, Token, parse_macro_input,
    parse::{Parse, ParseStream},
};

/// Per-field directive parsed from `#[fleet(...)]`.
enum FieldDirective {
    /// `#[fleet]` — source field name = struct field name.
    ByName { copy: bool },
    /// `#[fleet(<src>)]` — explicit FleetDefaults source field.
    Source { src: Ident, copy: bool },
    /// `#[fleet(with = path)]` — `field: path(fd)`.
    With { path: Path },
    /// `#[fleet(default = expr)]` — `field: expr`.
    Default { expr: Expr },
    /// `#[fleet(skip)]` — taken from the base ctor (struct-update).
    Skip,
}

/// Struct-level config parsed from `#[fleet(...)]` on the item.
struct StructConfig {
    base: Path,
    finalize: Option<Path>,
    copy_default: bool,
}

impl Default for StructConfig {
    fn default() -> Self {
        // `Default::default()` is the zero-config base ctor.
        Self {
            base: syn::parse_str::<Path>("::core::default::Default::default")
                .expect("static path parses"),
            finalize: None,
            copy_default: false,
        }
    }
}

/// One parsed `name = value | flag` inside a `#[fleet(...)]` list.
enum MetaItem {
    Flag(Ident),
    NameVal(Ident, MetaVal),
}

enum MetaVal {
    Path(Path),
    Expr(Expr),
    Str(syn::LitStr),
}

impl Parse for MetaItem {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let key: Ident = input.parse()?;
        if input.peek(Token![=]) {
            let _: Token![=] = input.parse()?;
            // `with`/`finalize`/`base` take a path-or-string; `default` an expr.
            if key == "default" {
                let expr: Expr = input.parse()?;
                Ok(MetaItem::NameVal(key, MetaVal::Expr(expr)))
            } else if input.peek(syn::LitStr) {
                let s: syn::LitStr = input.parse()?;
                Ok(MetaItem::NameVal(key, MetaVal::Str(s)))
            } else {
                let p: Path = input.parse()?;
                Ok(MetaItem::NameVal(key, MetaVal::Path(p)))
            }
        } else {
            Ok(MetaItem::Flag(key))
        }
    }
}

struct FleetArgs(Vec<MetaItem>);

impl Parse for FleetArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let items = input.parse_terminated(MetaItem::parse, Token![,])?;
        Ok(FleetArgs(items.into_iter().collect()))
    }
}

fn parse_field_directive(
    attr: &syn::Attribute,
    copy_default: bool,
) -> syn::Result<FieldDirective> {
    // Bare `#[fleet]` with no parens → by-name.
    if matches!(attr.meta, syn::Meta::Path(_)) {
        return Ok(FieldDirective::ByName { copy: copy_default });
    }
    let args: FleetArgs = attr.parse_args()?;
    let mut src: Option<Ident> = None;
    let mut copy = copy_default;
    let mut with: Option<Path> = None;
    let mut default: Option<Expr> = None;
    let mut skip = false;
    for item in args.0 {
        match item {
            MetaItem::Flag(f) if f == "copy" => copy = true,
            MetaItem::Flag(f) if f == "clone" => copy = false,
            MetaItem::Flag(f) if f == "skip" => skip = true,
            MetaItem::Flag(other) => {
                // A lone identifier is the source-field name.
                src = Some(other);
            }
            MetaItem::NameVal(k, MetaVal::Path(p)) if k == "with" => with = Some(p),
            MetaItem::NameVal(k, MetaVal::Expr(e)) if k == "default" => default = Some(e),
            MetaItem::NameVal(k, _) => {
                return Err(syn::Error::new_spanned(
                    k.clone(),
                    "unknown #[fleet(...)] field key (expected with = / default =)",
                ));
            }
        }
    }
    if skip {
        return Ok(FieldDirective::Skip);
    }
    if let Some(path) = with {
        return Ok(FieldDirective::With { path });
    }
    if let Some(expr) = default {
        return Ok(FieldDirective::Default { expr });
    }
    if let Some(src) = src {
        return Ok(FieldDirective::Source { src, copy });
    }
    Ok(FieldDirective::ByName { copy })
}

fn parse_struct_config(input: &DeriveInput) -> syn::Result<StructConfig> {
    let mut cfg = StructConfig::default();
    for attr in &input.attrs {
        if !attr.path().is_ident("fleet") {
            continue;
        }
        let args: FleetArgs = attr.parse_args()?;
        for item in args.0 {
            match item {
                MetaItem::Flag(f) if f == "copy_default" => cfg.copy_default = true,
                MetaItem::NameVal(k, MetaVal::Str(s)) if k == "base" => {
                    cfg.base = s.parse()?;
                }
                MetaItem::NameVal(k, MetaVal::Path(p)) if k == "base" => cfg.base = p,
                MetaItem::NameVal(k, MetaVal::Str(s)) if k == "finalize" => {
                    cfg.finalize = Some(s.parse()?);
                }
                MetaItem::NameVal(k, MetaVal::Path(p)) if k == "finalize" => {
                    cfg.finalize = Some(p);
                }
                MetaItem::Flag(other) => {
                    return Err(syn::Error::new_spanned(
                        other,
                        "unknown struct-level #[fleet(...)] flag",
                    ));
                }
                MetaItem::NameVal(k, _) => {
                    return Err(syn::Error::new_spanned(
                        k,
                        "unknown struct-level #[fleet(...)] key (expected base = / finalize =)",
                    ));
                }
            }
        }
    }
    Ok(cfg)
}

/// The pure token-emitting core — unit-testable without `proc_macro`.
fn expand(input: &DeriveInput) -> syn::Result<TokenStream2> {
    let self_name = &input.ident;
    let cfg = parse_struct_config(input)?;

    let fields = match &input.data {
        Data::Struct(DataStruct {
            fields: Fields::Named(named),
            ..
        }) => &named.named,
        _ => {
            return Err(syn::Error::new_spanned(
                self_name,
                "FleetThemed requires a named-fields struct",
            ));
        }
    };

    let mut assignments: Vec<TokenStream2> = Vec::new();
    for f in fields {
        let field_name = f.ident.as_ref().expect("named field has ident");
        let fleet_attr = f.attrs.iter().find(|a| a.path().is_ident("fleet"));
        let directive = match fleet_attr {
            Some(attr) => parse_field_directive(attr, cfg.copy_default)?,
            None => FieldDirective::Skip,
        };
        match directive {
            FieldDirective::Skip => { /* falls through to ..base */ }
            FieldDirective::ByName { copy } => {
                let rhs = if copy {
                    quote! { fd.#field_name }
                } else {
                    quote! { fd.#field_name.clone() }
                };
                assignments.push(quote! { #field_name: #rhs });
            }
            FieldDirective::Source { src, copy } => {
                let rhs = if copy {
                    quote! { fd.#src }
                } else {
                    quote! { fd.#src.clone() }
                };
                assignments.push(quote! { #field_name: #rhs });
            }
            FieldDirective::With { path } => {
                assignments.push(quote! { #field_name: #path(fd) });
            }
            FieldDirective::Default { expr } => {
                assignments.push(quote! { #field_name: #expr });
            }
        }
    }

    let base = &cfg.base;
    let ctor = quote! {
        Self {
            #(#assignments,)*
            ..#base()
        }
    };

    let body = match &cfg.finalize {
        Some(finalize) => quote! {
            let mut __cfg: Self = #ctor;
            #finalize(&mut __cfg, fd);
            __cfg
        },
        None => quote! { #ctor },
    };

    Ok(quote! {
        impl ::ishou_tokens::FleetThemedConfig for #self_name {
            fn from_fleet(fd: &::ishou_tokens::FleetDefaults) -> Self {
                #body
            }
        }
    })
}

#[proc_macro_derive(FleetThemed, attributes(fleet))]
pub fn derive_fleet_themed(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand(&input) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

#[cfg(test)]
mod tests {
    use super::expand;
    use syn::DeriveInput;
    use tatara_rust_snapshot::assert_tokens_contain;

    /// Run the pure emitter on a struct literal and return the source
    /// string for token-stability assertions.
    fn emit(src: &str) -> String {
        let di: DeriveInput = syn::parse_str(src).expect("test input parses");
        expand(&di).expect("expand succeeds").to_string()
    }

    /// Token-level "does NOT contain" check for negative assertions.
    fn canonical_contains(actual: &str, needle: &str) -> bool {
        let a = tatara_rust_snapshot::tokens::canonical_tokens(actual);
        let n = tatara_rust_snapshot::tokens::canonical_tokens(needle);
        a.contains(&n)
    }

    #[test]
    fn by_name_clones_string_field() {
        let out = emit(
            "struct C { #[fleet] theme: String, shell: ShellConfig }",
        );
        // by-name String → fd.theme.clone(); the `impl` head is fully qualified.
        assert_tokens_contain!(
            &out,
            "impl ::ishou_tokens::FleetThemedConfig for C",
        );
        // The full `from_fleet` signature, written with the same `& ::`
        // spacing the emitter produces so the bare-fragment tokenizer
        // matches the in-context tokens (a bare `&::` fragment is
        // tokenized ambiguously otherwise).
        assert_tokens_contain!(
            &out,
            "fn from_fleet(fd: & ::ishou_tokens::FleetDefaults) -> Self",
        );
        assert_tokens_contain!(&out, "theme: fd.theme.clone()");
        // the unannotated `shell` field falls through to the base ctor
        // (`..::` spacing matches the in-context token stream).
        assert_tokens_contain!(&out, ".. ::core::default::Default::default()");
    }

    #[test]
    fn explicit_source_and_copy_scalar() {
        let out = emit(
            "struct C { #[fleet(font_family)] family: String, #[fleet(font_size, copy)] size: f32 }",
        );
        assert_tokens_contain!(&out, "family: fd.font_family.clone()");
        assert_tokens_contain!(&out, "size: fd.font_size");
    }

    #[test]
    fn with_escape_hatch_calls_fn() {
        let out = emit("struct C { #[fleet(with = map_cursor)] cursor: CursorStyle }");
        assert_tokens_contain!(&out, "cursor: map_cursor(fd)");
    }

    #[test]
    fn default_literal_emits_expr() {
        let out = emit("struct C { #[fleet(default = 80u32)] cols: u32 }");
        assert_tokens_contain!(&out, "cols: 80u32");
    }

    #[test]
    fn skip_field_omitted_from_assignments() {
        let out = emit("struct C { #[fleet(skip)] profiles: P, #[fleet] theme: String }");
        // skip means NO `profiles:` assignment — it comes from the base.
        assert!(!canonical_contains(&out, "profiles : fd"));
        assert_tokens_contain!(&out, "theme: fd.theme.clone()");
    }

    #[test]
    fn struct_base_and_finalize() {
        let out = emit(
            "#[fleet(base = \"C::bare\", finalize = post)] struct C { #[fleet] theme: String }",
        );
        assert_tokens_contain!(&out, "..C::bare()");
        assert_tokens_contain!(&out, "let mut __cfg: Self =");
        assert_tokens_contain!(&out, "post(&mut __cfg, fd)");
    }

    #[test]
    fn copy_default_struct_attr_skips_clone() {
        let out = emit(
            "#[fleet(copy_default)] struct C { #[fleet(font_size)] size: f32 }",
        );
        assert_tokens_contain!(&out, "size: fd.font_size");
        assert!(!canonical_contains(&out, "fd.font_size.clone()"));
    }

    #[test]
    fn rejects_non_struct() {
        let di: DeriveInput = syn::parse_str("enum E { A, B }").unwrap();
        assert!(expand(&di).is_err());
    }
}
