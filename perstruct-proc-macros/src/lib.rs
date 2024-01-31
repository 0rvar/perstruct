//! This crate contains the proc macros for the [`perstruct`](https://crates.io/crates/perstruct) crate.
//! It is not intended to be used directly.

use quote::quote;
use quote::ToTokens;
use syn::{parse_macro_input, ItemStruct};

#[proc_macro_attribute]
pub fn perstruct(
    _args: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let input: ItemStruct = parse_macro_input!(input as ItemStruct);
    process_struct(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

fn process_struct(mut input: ItemStruct) -> syn::Result<proc_macro2::TokenStream> {
    if input.generics.params.len() != 0 {
        panic!("Struct cannot be generic");
    }

    let mut fields = vec![];
    let mut skipped_fields = vec![];
    for field in input.fields.iter_mut() {
        let ident = field.ident.clone().unwrap();
        let mut to_remove: Vec<syn::Path> = vec![];
        let mut key: Option<String> = None;
        let mut default_fn = None;
        let mut default_lit = None;
        let mut skip = false;

        for attr in &field.attrs {
            let attr_path = attr.path().clone();
            if attr_path.is_ident("perstruct") {
                to_remove.push(attr_path);
                let meta = attr.parse_args()?;
                match meta {
                    syn::Meta::NameValue(syn::MetaNameValue {
                        path,
                        value: syn::Expr::Lit(lit),
                        ..
                    }) => match path {
                        p if p.is_ident("key") => {
                            if let syn::Lit::Str(s) = lit.lit {
                                key = Some(s.value());
                            } else {
                                return Err(syn::Error::new_spanned(
                                    lit,
                                    "Expected string literal",
                                ));
                            }
                        }
                        p if p.is_ident("default_fn") => {
                            if let syn::Lit::Str(s) = lit.lit {
                                default_fn = Some(s.value());
                            } else {
                                return Err(syn::Error::new_spanned(
                                    lit,
                                    "Expected string literal",
                                ));
                            }
                        }
                        p if p.is_ident("default") => {
                            default_lit = Some(lit.lit);
                        }
                        thing => return Err(syn::Error::new_spanned(
                            thing.into_token_stream(),
                            "Unknown perstruct attribute (available: key, default_fn, default, skip)",
                        )),
                    },
                    syn::Meta::Path(path) => {
                        if path.is_ident("skip") {
                            skip = true;
                        } else {
                            return Err(syn::Error::new_spanned(
                                    path.into_token_stream(),
                                    "Unknown perstruct attribute (available: key, default_fn, default, skip)",
                                ));
                        }
                    }
                    thing => {
                        return Err(syn::Error::new_spanned(
                            attr.into_token_stream(),
                            format!("Parse args failed: {thing:?}"),
                        ))
                    }
                }
            }
        }
        for attr in to_remove {
            field.attrs.retain(|a| a.path() != &attr);
        }
        if skip {
            skipped_fields.push(ident);
            continue;
        }
        field.vis = syn::Visibility::Inherited;
        let ty = field.ty.clone();
        fields.push(PerstructField {
            ident,
            key,
            default_fn,
            default_lit,
            ty,
        });
    }

    // Add _perstruct_dirty_fields field
    let syn::Fields::Named(syn::FieldsNamed { named, .. }) = &mut input.fields else {
        return Err(syn::Error::new_spanned(
            input.ident,
            "Perstruct: struct must have named fields",
        ));
    };
    named.push(syn::Field {
        attrs: vec![],
        vis: syn::Visibility::Inherited,
        mutability: syn::FieldMutability::None,
        ident: Some(syn::Ident::new(
            "_perstruct_dirty_fields",
            proc_macro2::Span::mixed_site(),
        )),
        colon_token: None,
        ty: syn::Type::Verbatim(quote! { std::collections::HashSet<&'static str> }),
    });

    let ident = input.ident.clone();
    let default_impl = generate_default_impl(&ident, &fields, &skipped_fields);
    let methods_impl = generate_methods_impl(&ident, &fields);
    let keys = fields.iter().map(|field| {
        let key = field.key.clone().unwrap_or(field.ident.to_string());
        syn::LitStr::new(&key, proc_macro2::Span::mixed_site())
    });

    let from_map_impl = generate_from_map_impl(&fields);
    let get_changes_impl = generate_get_changes_impl(&fields);

    let tokens = quote::quote! {
        #input

        #default_impl

        #methods_impl

        impl #ident {
            pub fn perstruct_dirty_fields(&self) -> &std::collections::HashSet<&str> {
                &self._perstruct_dirty_fields
            }
            pub fn perstruct_keys() -> std::vec::Vec<&'static str> {
                vec![#( #keys ),*]
            }
            #from_map_impl
            #get_changes_impl
        }
    };
    Ok(tokens)
}

fn generate_get_changes_impl(fields: &[PerstructField]) -> proc_macro2::TokenStream {
    let match_arms = fields
        .iter()
        .map(|field| {
            let ident = &field.ident;
            let key = field.key.clone().unwrap_or(field.ident.to_string());
            let key_lit = syn::LitStr::new(&key, proc_macro2::Span::mixed_site());
            quote! {
                #key_lit => {
                    let value = serde_json::to_string(&self.#ident).map_err(|e| e.to_string())?;
                    changes.push((#key_lit, value));
                }
            }
        })
        .collect::<Vec<_>>();
    quote! {
        pub fn perstruct_get_changes(&self) -> Result<std::vec::Vec<(&'static str, String)>, String> {
            let mut changes = vec![];
            for key in self._perstruct_dirty_fields.iter() {
                match *key {
                    #(#match_arms)*,
                    _ => {}
                }
            }
            Ok(changes)
        }
        pub fn perstruct_saved(&mut self) {
            self._perstruct_dirty_fields.clear();
        }
    }
}

fn generate_from_map_impl(fields: &[PerstructField]) -> proc_macro2::TokenStream {
    let field_match_arms = fields
        .iter()
        .map(|field| {
            let key = field.key.clone().unwrap_or(field.ident.to_string());
            let key_lit = syn::LitStr::new(&key, proc_macro2::Span::mixed_site());
            let ty = &field.ty;
            let ident = &field.ident;
            quote! {
                #key_lit => {
                    match serde_json::from_str::<#ty>(value.as_ref()) {
                        Ok(json_value) => {
                            struct_value.#ident = json_value;
                            dirty_fields.remove(#key_lit);
                        }
                        Err(e) => {
                            deserialization_errors.push((#key_lit, e.to_string()));
                        }
                    }
                }
            }
        })
        .collect::<Vec<_>>();
    let keys = fields.iter().map(|field| {
        let key = field.key.clone().unwrap_or(field.ident.to_string());
        syn::LitStr::new(&key, proc_macro2::Span::mixed_site())
    });
    quote! {
        pub fn from_map<TKey, TValue>(
            map: &std::collections::HashMap<TKey, TValue>
        ) -> ::perstruct::PerstructLoadResult<Self>
            where TKey: std::convert::AsRef<str>,
                  TValue: std::convert::AsRef<str>
        {
            let mut dirty_fields = vec![
                #( #keys ),*
            ].into_iter().collect::<std::collections::HashSet<&'static str>>();
            let mut unknown_fields = vec![];

            let mut struct_value = Self::default();
            let mut deserialization_errors = vec![];
            for (key, value) in map.iter() {
                let key_ref: &str = key.as_ref();
                match key_ref {
                    #(#field_match_arms)*,
                    unknown_key => {
                        unknown_fields.push(unknown_key.to_string());
                    }
                }
            }
            struct_value._perstruct_dirty_fields = dirty_fields;
            ::perstruct::PerstructLoadResult {
                value: struct_value,
                deserialization_errors,
                unknown_fields,

            }
        }
    }
}

fn generate_methods_impl(
    ident: &syn::Ident,
    fields: &[PerstructField],
) -> proc_macro2::TokenStream {
    let methods = fields.iter().map(|field| {
        let ident = &field.ident;
        let ty = &field.ty;
        let (reference_return, reference_ty) = match ty {
            // copy types should be returned by value - all integer, float, bool, char
            syn::Type::Path(syn::TypePath { qself: None, path }) if path.segments.len() == 1 => {
                let segment = &path.segments[0];
                match segment.ident.to_string().as_str() {
                    "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8" | "u16" | "u32"
                    | "u64" | "u128" | "usize" | "f32" | "f64" | "bool" | "char" => {
                        (quote! { self.#ident }, quote! { #ty })
                    }
                    _ => (quote! { &self.#ident }, quote! { &#ty }),
                }
            }
            _ => (quote! { &self.#ident }, quote! { &#ty }),
        };
        let set_ident = syn::Ident::new(&format!("set_{}", ident), ident.span());
        let key = field.key.clone().unwrap_or(field.ident.to_string());
        let key_lit = syn::ExprLit {
            attrs: vec![],
            lit: syn::Lit::Str(syn::LitStr::new(&key.to_string(), ident.span())),
        };
        let update_ident = syn::Ident::new(&format!("update_{}", ident), ident.span());
        quote! {
            pub fn #ident(&self) -> #reference_ty {
                #reference_return
            }
            pub fn #set_ident(&mut self, value: #ty) {
                self.#ident = value;
                self._perstruct_dirty_fields.insert(#key_lit);
            }
            pub fn #update_ident(&mut self, f: impl FnOnce(&mut #ty)) {
                f(&mut self.#ident);
                self._perstruct_dirty_fields.insert(#key_lit);
            }
        }
    });
    quote::quote! {
        impl #ident {
            #(#methods)*
        }
    }
}

fn generate_default_impl(
    ident: &syn::Ident,
    fields: &[PerstructField],
    skipped_fields: &[syn::Ident],
) -> proc_macro2::TokenStream {
    let default_fields = fields.iter().map(|field| {
        let ident = &field.ident;
        if let Some(default_fn) = &field.default_fn {
            let default_fn = syn::Ident::new(default_fn, ident.span());
            quote::quote! { #ident: #default_fn() }
        } else if let Some(default_lit) = &field.default_lit {
            quote::quote! { #ident: #default_lit }
        } else {
            quote::quote! { #ident: Default::default() }
        }
    });
    let default_skipped_fields = skipped_fields.iter().map(|ident| {
        quote::quote! { #ident: Default::default() }
    });
    quote::quote! {
        #[automatically_derived]
        impl Default for #ident {
            fn default() -> Self {
                Self {
                    _perstruct_dirty_fields: Default::default(),
                    #(#default_fields),*,
                    #(#default_skipped_fields),*
                }
            }
        }
    }
}

#[derive(Debug)]
struct PerstructField {
    ident: syn::Ident,
    key: Option<String>,
    default_fn: Option<String>,
    default_lit: Option<syn::Lit>,
    ty: syn::Type,
}
