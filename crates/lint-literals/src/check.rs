//! One source file: its magic numbers, and the modules it keeps in files of
//! their own (for `main.rs` to follow).

use proc_macro2::{Span, TokenStream, TokenTree};
use syn::punctuated::Punctuated;
use syn::visit::{self, Visit};
use syn::{Attribute, Expr, Item, Lit, Meta, Token};

/// Numbers too plain to need a name.
const PLAIN: [f64; 3] = [0.0, 1.0, 2.0];

/// Calls whose arguments are UI layout copied from design/screens (sizes in
/// pixels, colours): they mirror the mockups and DESIGN.md's tokens 1:1.
const LAYOUT_CALLS: [&str; 3] = ["px", "rgb", "rgba"];

/// The comment that keeps a number where it is, followed by why.
pub const MARKER: &str = "// literal:";

/// A number that needs a name, where it is (both counted from 1).
#[derive(Debug, PartialEq, Eq)]
pub struct Hit {
    pub line: usize,
    pub column: usize,
    pub text: String,
}

/// `mod name;`: a module in a file of its own, maybe with `#[path]`, inside
/// these inline modules of the file.
#[derive(Debug, PartialEq, Eq)]
pub struct ModDecl {
    pub name: String,
    pub path: Option<String>,
    pub inline: Vec<String>,
}

pub struct Report {
    pub hits: Vec<Hit>,
    pub mods: Vec<ModDecl>,
}

/// The magic numbers in `source`'s own code (not its tests), and the modules
/// it declares in other files (not its test modules).
pub fn check(source: &str) -> syn::Result<Report> {
    let file = syn::parse_file(source)?;
    let lines: Vec<&str> = source.lines().collect();
    let mut checker = Checker {
        lines: &lines,
        constant: 0,
        calls: Vec::new(),
        inline: Vec::new(),
        hits: Vec::new(),
        mods: Vec::new(),
    };
    checker.visit_file(&file);
    Ok(Report {
        hits: checker.hits,
        mods: checker.mods,
    })
}

struct Checker<'s> {
    lines: &'s [&'s str],
    /// Inside a definition (const, static, enum discriminant): naming it is
    /// what the definition does.
    constant: u32,
    /// The calls (and macros) around the current expression, innermost last.
    calls: Vec<String>,
    /// The inline modules (`mod name { .. }`) around the current item.
    inline: Vec<String>,
    hits: Vec<Hit>,
    mods: Vec<ModDecl>,
}

impl Checker<'_> {
    fn literal(&mut self, lit: &Lit, span: Span) {
        let (value, text) = match lit {
            Lit::Int(i) => (i.base10_parse::<f64>().ok(), i.to_string()),
            Lit::Float(f) => (f.base10_parse::<f64>().ok(), f.to_string()),
            _ => return,
        };
        let layout = self
            .calls
            .last()
            .is_some_and(|c| LAYOUT_CALLS.contains(&c.as_str()));
        if self.constant > 0 || layout || value.is_some_and(|v| PLAIN.contains(&v)) {
            return;
        }
        let start = span.start();
        // A reason on the number's line, or alone on the line above (where
        // rustfmt leaves it on a wrapped expression).
        if self.reason_on(start.line) || self.reason_above(start.line) {
            return;
        }
        self.hits.push(Hit {
            line: start.line,
            column: start.column + 1,
            text,
        });
    }

    /// Line `line` (from 1) has a `// literal: <reason>` with a reason.
    fn reason_on(&self, line: usize) -> bool {
        self.line(line)
            .and_then(|l| l.split_once(MARKER))
            .is_some_and(|(_, reason)| !reason.trim().is_empty())
    }

    /// The line above `line` is nothing but a `// literal: <reason>`.
    fn reason_above(&self, line: usize) -> bool {
        let above = line.saturating_sub(1);
        self.line(above)
            .is_some_and(|l| l.trim_start().starts_with(MARKER))
            && self.reason_on(above)
    }

    fn line(&self, line: usize) -> Option<&str> {
        line.checked_sub(1).and_then(|i| self.lines.get(i)).copied()
    }

    /// A whole item kept with one reason above it (data copied from
    /// elsewhere: the design's drawings, demo data, a protocol's table).
    /// The reason may stand above the item or anywhere among its attributes
    /// and doc comments, up to the line it starts on.
    fn item_explained(&self, item: &impl syn::spanned::Spanned) -> bool {
        let start = item.span().start().line;
        let mut head = start;
        while self.line(head).is_some_and(|l| {
            let l = l.trim_start();
            l.starts_with("#[") || l.starts_with("//")
        }) {
            head += 1;
        }
        (start.saturating_sub(1)..head).any(|line| {
            self.line(line)
                .is_some_and(|l| l.trim_start().starts_with(MARKER))
                && self.reason_on(line)
        })
    }

    /// A macro that isn't a list of expressions (`vec![0; 512]`, `json!`):
    /// every number in its tokens.
    fn tokens(&mut self, tokens: TokenStream) {
        for token in tokens {
            match token {
                TokenTree::Literal(l) => {
                    let span = l.span();
                    self.literal(&Lit::new(l), span);
                }
                TokenTree::Group(g) => self.tokens(g.stream()),
                _ => {}
            }
        }
    }

    fn defining(&mut self, visit: impl FnOnce(&mut Self)) {
        self.constant += 1;
        visit(self);
        self.constant -= 1;
    }

    fn in_call(&mut self, name: String, visit: impl FnOnce(&mut Self)) {
        self.calls.push(name);
        visit(self);
        self.calls.pop();
    }

    fn module(&mut self, m: &syn::ItemMod) {
        match &m.content {
            Some((_, items)) => {
                self.inline.push(m.ident.to_string());
                for item in items {
                    self.visit_item(item);
                }
                self.inline.pop();
            }
            None => self.mods.push(ModDecl {
                name: m.ident.to_string(),
                path: path_attr(&m.attrs),
                inline: self.inline.clone(),
            }),
        }
    }
}

impl<'a> Visit<'a> for Checker<'_> {
    fn visit_item(&mut self, item: &'a Item) {
        if test_only(attrs_of(item)) || self.item_explained(item) {
            return;
        }
        match item {
            Item::Const(_) | Item::Static(_) => self.defining(|c| visit::visit_item(c, item)),
            Item::Mod(m) => self.module(m),
            _ => visit::visit_item(self, item),
        }
    }

    fn visit_impl_item(&mut self, item: &'a syn::ImplItem) {
        match item {
            _ if self.item_explained(item) => {}
            syn::ImplItem::Fn(f) if test_only(&f.attrs) => {}
            syn::ImplItem::Const(_) => self.defining(|c| visit::visit_impl_item(c, item)),
            _ => visit::visit_impl_item(self, item),
        }
    }

    fn visit_trait_item(&mut self, item: &'a syn::TraitItem) {
        match item {
            syn::TraitItem::Const(_) => self.defining(|c| visit::visit_trait_item(c, item)),
            _ => visit::visit_trait_item(self, item),
        }
    }

    fn visit_variant(&mut self, variant: &'a syn::Variant) {
        self.defining(|c| visit::visit_variant(c, variant));
    }

    fn visit_expr_call(&mut self, call: &'a syn::ExprCall) {
        let name = match &*call.func {
            Expr::Path(p) => p
                .path
                .segments
                .last()
                .map(|s| s.ident.to_string())
                .unwrap_or_default(),
            _ => String::new(),
        };
        self.in_call(name, |c| visit::visit_expr_call(c, call));
    }

    fn visit_expr_method_call(&mut self, call: &'a syn::ExprMethodCall) {
        self.in_call(call.method.to_string(), |c| {
            visit::visit_expr_method_call(c, call);
        });
    }

    fn visit_macro(&mut self, mac: &'a syn::Macro) {
        let name = mac
            .path
            .segments
            .last()
            .map(|s| format!("{}!", s.ident))
            .unwrap_or_default();
        self.in_call(name, |c| {
            match mac.parse_body_with(Punctuated::<Expr, Token![,]>::parse_terminated) {
                Ok(args) => args.iter().for_each(|e| c.visit_expr(e)),
                Err(_) => c.tokens(mac.tokens.clone()),
            }
        });
    }

    fn visit_lit(&mut self, lit: &'a Lit) {
        self.literal(lit, lit.span());
    }

    /// Attribute arguments (`#[serde(..)]`, `#[cfg(..)]`) aren't code.
    fn visit_attribute(&mut self, _: &'a Attribute) {}
}

/// `#[test]` (any test macro's: tokio's, gpui-kit's) or `#[cfg(test)]`,
/// also within `all(..)`.
fn test_only(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|a| {
        match a
            .path()
            .segments
            .last()
            .map(|s| s.ident.to_string())
            .as_deref()
        {
            Some("test") => true,
            Some("cfg") => a.parse_args::<Meta>().is_ok_and(|m| cfg_test(&m)),
            _ => false,
        }
    })
}

fn cfg_test(meta: &Meta) -> bool {
    match meta {
        Meta::Path(p) => p.is_ident("test"),
        Meta::List(l) if l.path.is_ident("all") => l
            .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
            .is_ok_and(|all| all.iter().any(cfg_test)),
        _ => false,
    }
}

fn path_attr(attrs: &[Attribute]) -> Option<String> {
    attrs.iter().find_map(|a| match &a.meta {
        Meta::NameValue(nv) if nv.path.is_ident("path") => match &nv.value {
            Expr::Lit(syn::ExprLit {
                lit: Lit::Str(s), ..
            }) => Some(s.value()),
            _ => None,
        },
        _ => None,
    })
}

fn attrs_of(item: &Item) -> &[Attribute] {
    match item {
        Item::Const(i) => &i.attrs,
        Item::Enum(i) => &i.attrs,
        Item::ExternCrate(i) => &i.attrs,
        Item::Fn(i) => &i.attrs,
        Item::ForeignMod(i) => &i.attrs,
        Item::Impl(i) => &i.attrs,
        Item::Macro(i) => &i.attrs,
        Item::Mod(i) => &i.attrs,
        Item::Static(i) => &i.attrs,
        Item::Struct(i) => &i.attrs,
        Item::Trait(i) => &i.attrs,
        Item::TraitAlias(i) => &i.attrs,
        Item::Type(i) => &i.attrs,
        Item::Union(i) => &i.attrs,
        Item::Use(i) => &i.attrs,
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hits(source: &str) -> Vec<String> {
        check(source)
            .unwrap()
            .hits
            .into_iter()
            .map(|h| h.text)
            .collect()
    }

    const NONE: [&str; 0] = [];

    #[test]
    fn finds_magic_numbers_but_not_plain_ones() {
        assert_eq!(hits("fn f() -> u32 { 42 }"), ["42"]);
        assert_eq!(hits("fn f() { g(0, 1, 2, 2.0, -1) }"), NONE);
        assert_eq!(hits("fn f() { let b = [0u8; 512]; }"), ["512"]);
        assert_eq!(hits("fn f() { h(0x8180, 0.5) }"), ["0x8180", "0.5"]);
        let at = check("fn f() {\n    g(60)\n}").unwrap().hits;
        assert_eq!((at[0].line, at[0].column), (2, 7));
    }

    #[test]
    fn definitions_name_their_numbers() {
        assert_eq!(hits("const WAIT: u64 = 300;"), NONE);
        assert_eq!(hits("static BUF: [u8; 64] = [7; 64];"), NONE);
        assert_eq!(hits("impl X { const N: usize = 9; }"), NONE);
        assert_eq!(hits("enum Code { Refused = 5 }"), NONE);
    }

    #[test]
    fn ui_layout_is_the_design() {
        assert_eq!(hits("fn f() { div().mt(px(10.)).bg(rgb(0xFFFFFF)) }"), NONE);
        assert_eq!(hits("fn f() { div().w(px(W - 20.)) }"), NONE);
        assert_eq!(hits("fn f() { div().opacity(0.5) }"), ["0.5"]);
    }

    #[test]
    fn tests_are_left_alone() {
        assert_eq!(hits("#[test]\nfn t() { assert_eq!(f(), 42); }"), NONE);
        assert_eq!(hits("#[tokio::test]\nasync fn t() { g(42) }"), NONE);
        assert_eq!(hits("#[cfg(test)]\nmod tests { fn h() { g(42) } }"), NONE);
        assert_eq!(
            hits("#[cfg(all(test, windows))]\nmod t { fn h() { g(42) } }"),
            NONE
        );
        assert_eq!(hits("#[cfg(windows)]\nmod w { fn h() { g(42) } }"), ["42"]);
        assert_eq!(hits("impl X {\n#[test]\nfn t() { g(42) }\n}"), NONE);
    }

    #[test]
    fn a_reason_keeps_a_number() {
        assert_eq!(hits("fn f() { g(53) } // literal: DNS's port"), NONE);
        assert_eq!(
            hits("fn f() {\n    // literal: DNS's port\n    g(53)\n}"),
            NONE
        );
        assert_eq!(hits("fn f() { g(53) } // literal:"), ["53"], "no reason");
        assert_eq!(
            hits("fn f() { g(53) } // literal:   "),
            ["53"],
            "blank reason"
        );
        assert_eq!(hits("fn f() {\n    // see below\n    g(53)\n}"), ["53"]);
    }

    #[test]
    fn a_reason_above_an_item_keeps_all_of_it() {
        let art = "// literal: the design's drawing\nfn art() { g(14., 13.); h(1.56) }";
        assert_eq!(hits(art), NONE);
        let attrs = "// literal: the design's drawing\n/// Doc.\n#[inline]\nfn art() { g(14.) }";
        assert_eq!(hits(attrs), NONE, "above its doc and attributes");
        let among = "/// Doc.\n// literal: the design's drawing\n#[inline]\nfn art() { g(14.) }";
        assert_eq!(hits(among), NONE, "among its doc and attributes");
        assert_eq!(
            hits("// literal:\nfn f() { g(14.) }"),
            ["14."],
            "a reason is needed"
        );
        let method = "impl X {\n    // literal: sample data\n    fn demo() { g(42) }\n    fn f() { g(9) }\n}";
        assert_eq!(hits(method), ["9"], "only the item under it");
        let module = check("// literal: the design's sample data\nmod demo;").unwrap();
        assert!(module.mods.is_empty(), "a kept module's file isn't read");
    }

    #[test]
    fn numbers_in_macros_count() {
        assert_eq!(hits(r#"fn f() { format!("{}", x / 60); }"#), ["60"]);
        assert_eq!(hits("fn f() { vec![0x12, 0x34]; }"), ["0x12", "0x34"]);
        assert_eq!(
            hits("fn f() { vec![0u8; 512]; }"),
            ["512"],
            "not an expression list"
        );
        assert_eq!(hits("fn f() { vec![px(10.)]; }"), NONE);
        assert_eq!(hits("#[serde(default = \"x\")]\nstruct S { a: u8 }"), NONE);
    }

    #[test]
    fn modules_in_other_files_are_followed() {
        let src = "mod a;\n#[cfg(test)]\nmod tests;\n#[path = \"menu.rs\"]\nmod tray;\nmod inner { mod deep; }";
        let mods = check(src).unwrap().mods;
        let names: Vec<_> = mods
            .iter()
            .map(|m| (m.name.as_str(), m.path.as_deref()))
            .collect();
        assert_eq!(
            names,
            [("a", None), ("tray", Some("menu.rs")), ("deep", None)]
        );
        assert_eq!(mods[2].inline, ["inner"]);
    }
}
