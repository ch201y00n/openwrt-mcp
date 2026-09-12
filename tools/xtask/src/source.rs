use std::collections::{BTreeMap, BTreeSet};

use proc_macro2::{TokenStream, TokenTree};
use syn::{
    Attribute, Item, Path, UseTree,
    visit::{self, Visit},
};

use crate::{CheckResult, CrateRule};

type Aliases = BTreeMap<String, BTreeSet<String>>;

fn ident_text(ident: &syn::Ident) -> String {
    ident.to_string().trim_start_matches("r#").to_owned()
}

#[derive(Default)]
struct Imports {
    aliases: Aliases,
}

fn uses(tree: &UseTree, prefix: &str, result: &mut Vec<(String, String)>) {
    match tree {
        UseTree::Path(value) => uses(
            &value.tree,
            &format!("{prefix}{}::", ident_text(&value.ident)),
            result,
        ),
        UseTree::Name(value) => {
            let name = ident_text(&value.ident);
            let path = if name == "self" {
                prefix.trim_end_matches("::").to_owned()
            } else {
                format!("{prefix}{name}")
            };
            let local = path.rsplit("::").next().unwrap_or(&name).to_owned();
            result.push((local, path));
        }
        UseTree::Rename(value) => {
            let path = if value.ident == "self" {
                prefix.trim_end_matches("::").to_owned()
            } else {
                format!("{prefix}{}", ident_text(&value.ident))
            };
            result.push((ident_text(&value.rename), path));
        }
        UseTree::Glob(_) => result.push(("*".into(), prefix.trim_end_matches("::").into())),
        UseTree::Group(value) => {
            for item in &value.items {
                uses(item, prefix, result);
            }
        }
    }
}

impl<'ast> Visit<'ast> for Imports {
    fn visit_item(&mut self, item: &'ast Item) {
        if !test_only(item_attributes(item)) {
            visit::visit_item(self, item);
        }
    }
    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        let mut imports = Vec::new();
        uses(&item.tree, "", &mut imports);
        for (local, original) in imports {
            if local != "*" {
                self.aliases.entry(local).or_default().insert(original);
            }
        }
        visit::visit_item_use(self, item);
    }

    fn visit_item_extern_crate(&mut self, item: &'ast syn::ItemExternCrate) {
        let local = item
            .rename
            .as_ref()
            .map(|(_, alias)| alias)
            .unwrap_or(&item.ident);
        self.aliases
            .entry(ident_text(local))
            .or_default()
            .insert(ident_text(&item.ident));
    }
}

fn path_text(path: &Path) -> String {
    path.segments
        .iter()
        .map(|part| ident_text(&part.ident))
        .collect::<Vec<_>>()
        .join("::")
}

fn expands(path: &str, aliases: &Aliases) -> BTreeSet<String> {
    let mut seen = BTreeSet::new();
    let mut pending = vec![path.to_owned()];
    while let Some(path) = pending.pop() {
        if !seen.insert(path.clone()) {
            continue;
        }
        let (head, tail) = path
            .split_once("::")
            .map_or((path.as_str(), String::new()), |(head, tail)| {
                (head, format!("::{tail}"))
            });
        if let Some(targets) = aliases.get(head) {
            for target in targets {
                if target != head {
                    let candidate = format!("{target}{tail}");
                    // Recursive aliases are invalid Rust; bound expansion before compilation.
                    if candidate.len() <= 1024 && seen.len() < 1024 {
                        pending.push(candidate);
                    }
                }
            }
        }
    }
    seen
}

fn prefix(path: &str, boundary: &str) -> bool {
    path == boundary
        || path
            .strip_prefix(boundary)
            .is_some_and(|suffix| suffix.starts_with("::"))
}

struct Checker<'a> {
    rule: &'a CrateRule,
    aliases: Aliases,
    development: bool,
    errors: BTreeSet<String>,
}

impl Checker<'_> {
    fn check_path(&mut self, path: &str, glob: bool) {
        if self.development {
            return;
        }
        for expanded in expands(path, &self.aliases) {
            for forbidden in &self.rule.forbidden_paths {
                if prefix(&expanded, forbidden) || (glob && prefix(forbidden, &expanded)) {
                    self.errors.insert(format!("forbidden API {expanded}"));
                }
            }
        }
    }

    fn macro_name(&mut self, name: &str) {
        if self.development {
            return;
        }
        self.check_path(name, false);
        let expanded_names = expands(name, &self.aliases);
        for expanded in &expanded_names {
            if expanded == name && expanded_names.len() > 1 {
                continue;
            }
            let leaf = expanded.rsplit("::").next().unwrap_or(expanded);
            if matches!(
                leaf,
                "include" | "include_str" | "include_bytes" | "macro_rules" | "asm" | "global_asm"
            ) || !self
                .rule
                .allowed_macros
                .iter()
                .any(|allowed| allowed == leaf)
            {
                self.errors.insert(format!("unreviewed macro {expanded}"));
            }
        }
    }

    // syn intentionally does not expand macro bodies. Inspect all nested token
    // groups as well so an approved json!/format! cannot hide an explicit I/O path.
    fn tokens(&mut self, tokens: TokenStream) {
        let tokens: Vec<_> = tokens.into_iter().collect();
        let mut index = 0;
        while index < tokens.len() {
            if let TokenTree::Group(group) = &tokens[index] {
                self.tokens(group.stream());
            }
            if let TokenTree::Ident(ident) = &tokens[index] {
                if !self.development && matches!(ident_text(ident).as_str(), "use" | "extern") {
                    self.errors.insert(
                        "imports inside macro bodies require an explicit reviewed adapter".into(),
                    );
                }
                let mut path = ident_text(ident);
                let mut cursor = index + 1;
                while cursor + 2 < tokens.len() {
                    match (&tokens[cursor], &tokens[cursor + 1], &tokens[cursor + 2]) {
                        (
                            TokenTree::Punct(first),
                            TokenTree::Punct(second),
                            TokenTree::Ident(next),
                        ) if first.as_char() == ':' && second.as_char() == ':' => {
                            path.push_str("::");
                            path.push_str(&ident_text(next));
                            cursor += 3;
                        }
                        _ => break,
                    }
                }
                self.check_path(&path, false);
                if matches!(tokens.get(cursor), Some(TokenTree::Punct(value)) if value.as_char() == '!')
                {
                    self.macro_name(&path);
                }
                index = cursor;
            } else {
                index += 1;
            }
        }
    }
}

fn test_only(attributes: &[Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        attribute.path().is_ident("test")
            || path_text(attribute.path()) == "tokio::test"
            || (attribute.path().is_ident("cfg")
                && attribute
                    .parse_args::<syn::Path>()
                    .is_ok_and(|path| path.is_ident("test")))
    })
}

fn item_attributes(item: &Item) -> &[Attribute] {
    match item {
        Item::Const(v) => &v.attrs,
        Item::Enum(v) => &v.attrs,
        Item::ExternCrate(v) => &v.attrs,
        Item::Fn(v) => &v.attrs,
        Item::ForeignMod(v) => &v.attrs,
        Item::Impl(v) => &v.attrs,
        Item::Macro(v) => &v.attrs,
        Item::Mod(v) => &v.attrs,
        Item::Static(v) => &v.attrs,
        Item::Struct(v) => &v.attrs,
        Item::Trait(v) => &v.attrs,
        Item::TraitAlias(v) => &v.attrs,
        Item::Type(v) => &v.attrs,
        Item::Union(v) => &v.attrs,
        Item::Use(v) => &v.attrs,
        _ => &[],
    }
}

impl<'ast> Visit<'ast> for Checker<'_> {
    fn visit_path(&mut self, path: &'ast Path) {
        self.check_path(&path_text(path), false);
        visit::visit_path(self, path);
    }

    fn visit_item(&mut self, item: &'ast Item) {
        let attributes = item_attributes(item);
        let previous = self.development;
        self.development |= test_only(attributes);
        if !self.development {
            let unsafe_or_ffi = match item {
                Item::ForeignMod(_) => true,
                Item::Fn(item) => item.sig.unsafety.is_some() || item.sig.abi.is_some(),
                Item::Impl(item) => item.unsafety.is_some(),
                Item::Trait(item) => item.unsafety.is_some(),
                _ => false,
            };
            if unsafe_or_ffi {
                self.errors
                    .insert("unsafe/FFI items require a new architecture".into());
            }
        }
        visit::visit_item(self, item);
        self.development = previous;
    }

    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        let mut imports = Vec::new();
        uses(&item.tree, "", &mut imports);
        for (_, path) in imports {
            // Broad namespaces can be re-exported through another module and
            // evade file-local name resolution. Require imports below a boundary.
            self.check_path(&path, true);
        }
        // Rust use trees are not represented as syn::Path.
        for attribute in &item.attrs {
            self.visit_attribute(attribute);
        }
    }

    fn visit_item_extern_crate(&mut self, item: &'ast syn::ItemExternCrate) {
        self.check_path(&ident_text(&item.ident), true);
    }

    fn visit_expr_unsafe(&mut self, expression: &'ast syn::ExprUnsafe) {
        if !self.development {
            self.errors
                .insert("unsafe expressions require a new architecture".into());
        }
        visit::visit_expr_unsafe(self, expression);
    }

    fn visit_macro(&mut self, value: &'ast syn::Macro) {
        self.macro_name(&path_text(&value.path));
        self.tokens(value.tokens.clone());
    }

    fn visit_attribute(&mut self, attribute: &'ast Attribute) {
        let path = path_text(attribute.path());
        // Never accept remapped files, even in development targets; otherwise a
        // fixture could place production source beyond its package ownership.
        if path == "path" {
            self.errors
                .insert("#[path] module remapping is forbidden".into());
        }
        if path == "cfg_attr" {
            let parsed = attribute.parse_args_with(
                syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
            );
            match parsed {
                Ok(values) => {
                    for nested in values.iter().skip(1) {
                        let synthetic: Attribute = syn::parse_quote!(#[#nested]);
                        self.visit_attribute(&synthetic);
                    }
                }
                Err(_) => {
                    self.errors.insert("unparsed cfg_attr".into());
                }
            }
        }
        if !self.development {
            if let syn::Meta::List(list) = &attribute.meta {
                self.tokens(list.tokens.clone());
            } else if let syn::Meta::NameValue(value) = &attribute.meta {
                self.visit_expr(&value.value);
            }
            if path == "serde" {
                let parsed = attribute.parse_args_with(
                    syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
                );
                match parsed {
                    Ok(values) => {
                        for value in values {
                            if let syn::Meta::NameValue(value) = value
                                && [
                                    "default",
                                    "with",
                                    "serialize_with",
                                    "deserialize_with",
                                    "skip_serializing_if",
                                    "getter",
                                    "remote",
                                    "from",
                                    "try_from",
                                    "into",
                                    "crate",
                                ]
                                .contains(&path_text(&value.path).as_str())
                            {
                                match &value.value {
                                    syn::Expr::Lit(literal) => match &literal.lit {
                                        syn::Lit::Str(literal) => {
                                            match syn::parse_str::<syn::Path>(&literal.value()) {
                                                Ok(path) => {
                                                    self.check_path(&path_text(&path), false)
                                                }
                                                Err(_) => {
                                                    self.errors.insert("unparsed serde callback/type path requires review".into());
                                                }
                                            }
                                        }
                                        _ => {
                                            self.errors.insert(
                                                "serde callback/type path must be a string".into(),
                                            );
                                        }
                                    },
                                    _ => {
                                        self.errors.insert(
                                            "unparsed serde callback expression requires review"
                                                .into(),
                                        );
                                    }
                                }
                            }
                        }
                    }
                    Err(_) => {
                        self.errors
                            .insert("unparsed serde attribute requires review".into());
                    }
                }
            }
            let allowed = [
                "derive",
                "default",
                "cfg",
                "cfg_attr",
                "doc",
                "allow",
                "warn",
                "deny",
                "forbid",
                "expect",
                "inline",
                "cold",
                "must_use",
                "repr",
                "non_exhaustive",
                "deprecated",
                "serde",
                "error",
                "from",
                "source",
                "backtrace",
                "async_trait",
                "async_trait::async_trait",
            ];
            if !allowed.contains(&path.as_str()) {
                self.errors.insert(format!("unreviewed attribute {path}"));
            }
            if path == "derive" {
                let derives = attribute.parse_args_with(
                    syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
                );
                match derives {
                    Ok(values) => {
                        for value in values {
                            let name = path_text(&value);
                            let leaf = name.rsplit("::").next().unwrap_or(&name);
                            if ![
                                "Clone",
                                "Copy",
                                "Debug",
                                "Default",
                                "PartialEq",
                                "Eq",
                                "PartialOrd",
                                "Ord",
                                "Hash",
                                "Deserialize",
                                "Serialize",
                                "Error",
                            ]
                            .contains(&leaf)
                            {
                                self.errors.insert(format!("unreviewed derive {name}"));
                            }
                            self.check_path(&name, false);
                        }
                    }
                    Err(_) => {
                        self.errors.insert("unparsed derives".into());
                    }
                }
            }
        }
    }
}

/// Parse all cfg branches. Explicit test modules/targets may use fixture I/O.
/// This is a conservative AST boundary check, not procedural-macro expansion or
/// a proof of Rust name/type resolution; Cargo and behavioral tests complement it.
pub fn check_source(rule: &CrateRule, source: &str, development: bool) -> CheckResult {
    check_source_with_aliases(rule, source, development, &BTreeMap::new())
}

/// Include Cargo's crate-renaming map so an allowed dependency cannot hide a
/// forbidden API behind a manifest alias (including target-specific aliases).
pub fn check_source_with_aliases(
    rule: &CrateRule,
    source: &str,
    development: bool,
    dependency_aliases: &BTreeMap<String, String>,
) -> CheckResult {
    let ast = syn::parse_file(source).map_err(|_| "invalid Rust source")?;
    let mut imports = Imports::default();
    imports.visit_file(&ast);
    for (alias, original) in dependency_aliases {
        imports
            .aliases
            .entry(alias.clone())
            .or_default()
            .insert(original.clone());
    }
    let mut checker = Checker {
        rule,
        aliases: imports.aliases,
        development,
        errors: BTreeSet::new(),
    };
    checker.visit_file(&ast);
    if checker.errors.is_empty() {
        Ok(())
    } else {
        Err(checker.errors.into_iter().collect::<Vec<_>>().join("; "))
    }
}

/// Keep producer-side public aliases from flattening a namespace forbidden to a
/// consumer by the architecture contract. This complements, rather than replaces,
/// Rust visibility and tests of what secret-bearing values actually disclose.
pub fn check_public_reexports(
    source: &str,
    protected: &BTreeSet<String>,
    crate_prefix: &str,
) -> CheckResult {
    if protected.is_empty() {
        return Ok(());
    }
    let ast = syn::parse_file(source).map_err(|_| "invalid Rust source")?;
    let mut imports = Imports::default();
    imports.visit_file(&ast);
    struct PublicSurface<'a> {
        protected: &'a BTreeSet<String>,
        crate_prefix: &'a str,
        aliases: &'a Aliases,
        errors: BTreeSet<String>,
        inspect_paths: bool,
    }
    impl PublicSurface<'_> {
        fn path(&mut self, path: &str) {
            for expanded in expands(path, self.aliases) {
                let mut local = expanded
                    .strip_prefix(self.crate_prefix)
                    .unwrap_or(&expanded);
                while let Some(rest) = local
                    .strip_prefix("crate::")
                    .or_else(|| local.strip_prefix("self::"))
                    .or_else(|| local.strip_prefix("super::"))
                {
                    local = rest;
                }
                if self
                    .protected
                    .iter()
                    .any(|protected| prefix(local, protected))
                {
                    self.errors.insert(format!(
                        "public surface flattens protected namespace {local}"
                    ));
                }
            }
        }
    }
    impl<'ast> Visit<'ast> for PublicSurface<'_> {
        fn visit_item(&mut self, item: &'ast Item) {
            if test_only(item_attributes(item)) {
                return;
            }
            match item {
                Item::Use(item) if matches!(item.vis, syn::Visibility::Public(_)) => {
                    let mut imports = Vec::new();
                    uses(&item.tree, "", &mut imports);
                    for (_, path) in imports {
                        self.path(&path);
                    }
                }
                Item::Type(item) if matches!(item.vis, syn::Visibility::Public(_)) => {
                    self.inspect_paths = true;
                    self.visit_type(&item.ty);
                    self.inspect_paths = false;
                }
                Item::Fn(item) if matches!(item.vis, syn::Visibility::Public(_)) => {
                    self.inspect_paths = true;
                    self.visit_signature(&item.sig);
                    self.inspect_paths = false;
                }
                _ => visit::visit_item(self, item),
            }
        }
        fn visit_field(&mut self, field: &'ast syn::Field) {
            if matches!(field.vis, syn::Visibility::Public(_)) {
                self.inspect_paths = true;
                self.visit_type(&field.ty);
                self.inspect_paths = false;
            }
        }
        fn visit_path(&mut self, path: &'ast syn::Path) {
            if self.inspect_paths {
                self.path(&path_text(path));
            }
            visit::visit_path(self, path);
        }
    }
    let mut visitor = PublicSurface {
        protected,
        crate_prefix,
        aliases: &imports.aliases,
        errors: BTreeSet::new(),
        inspect_paths: false,
    };
    visitor.visit_file(&ast);
    if visitor.errors.is_empty() {
        Ok(())
    } else {
        Err(visitor.errors.into_iter().collect::<Vec<_>>().join("; "))
    }
}
