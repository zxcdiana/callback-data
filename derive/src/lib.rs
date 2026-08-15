use darling::FromMeta;
use darling::ast::NestedMeta;
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, Ident, Type, parse_macro_input};

const OWNER_ATTR: &str = "owner";

// ---------------------------------------------------------------------
// #[callback_data(prefix = "...")] — top-level type
// ---------------------------------------------------------------------

#[derive(FromMeta)]
struct CallbackDataArgs {
    prefix: String,
}

#[proc_macro_attribute]
pub fn callback_data(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attr_args = match NestedMeta::parse_meta_list(attr.into()) {
        Ok(v) => v,
        Err(e) => return darling::Error::from(e).write_errors().into(),
    };
    let args = match CallbackDataArgs::from_list(&attr_args) {
        Ok(v) => v,
        Err(e) => return e.write_errors().into(),
    };

    let mut input = parse_macro_input!(item as DeriveInput);
    let struct_name = input.ident.clone();
    let prefix = &args.prefix;

    if prefix.contains(':') {
        return syn::Error::new_spanned(
            &struct_name,
            format!("callback_data prefix `{prefix}` must not contain `:` (the token separator)"),
        )
        .to_compile_error()
        .into();
    }

    let fields = match extract_named_fields(&input) {
        Ok(f) => f,
        Err(e) => return e.to_compile_error().into(),
    };

    // `#[callback_data(owner)]` on a field is just a marker we already
    // consumed above — strip it before re-emitting the struct, otherwise
    // it collides with (and gets re-expanded as) this very attribute macro.
    strip_callback_data_field_attrs(&mut input);

    let flat_field_impl = flat_field_impl_for_struct(&struct_name, &fields);
    let owner_id_body = owner_id_body(&struct_name, &fields);

    let expanded = quote! {
        #input

        impl callback_data::CallbackDataConfig for #struct_name {
            const PREFIX: &'static str = #prefix;
        }

        #flat_field_impl

        impl callback_data::CallbackDataTrait for #struct_name {
            fn owner_id(&self) -> ::std::option::Option<i64> {
                #owner_id_body
            }
        }
    };

    expanded.into()
}

// ---------------------------------------------------------------------
// #[derive(FlatField)] — nested structs & enums
// ---------------------------------------------------------------------

#[proc_macro_derive(FlatField, attributes(callback_data))]
pub fn derive_flat_field(item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    let name = &input.ident;

    let expanded = match &input.data {
        Data::Struct(_) => {
            let fields = match extract_named_fields(&input) {
                Ok(f) => f,
                Err(e) => return e.to_compile_error().into(),
            };
            flat_field_impl_for_struct(name, &fields)
        }
        Data::Enum(data_enum) => flat_field_impl_for_enum(name, data_enum),
        Data::Union(_) => syn::Error::new_spanned(name, "FlatField cannot be derived for unions")
            .to_compile_error(),
    };

    expanded.into()
}

// ---------------------------------------------------------------------
// shared helpers
// ---------------------------------------------------------------------

struct FieldInfo {
    ident: Ident,
    ty: Type,
    is_owner: bool,
}

fn extract_named_fields(input: &DeriveInput) -> syn::Result<Vec<FieldInfo>> {
    let Data::Struct(data_struct) = &input.data else {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "expected a struct with named fields",
        ));
    };

    let Fields::Named(named) = &data_struct.fields else {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "callback-data structs must have named fields (no tuple/unit structs)",
        ));
    };

    named
        .named
        .iter()
        .map(|f| {
            let ident = f
                .ident
                .clone()
                .ok_or_else(|| syn::Error::new_spanned(f, "field must be named"))?;
            let is_owner = f.attrs.iter().any(|a| is_owner_attr(a));
            Ok(FieldInfo {
                ident,
                ty: f.ty.clone(),
                is_owner,
            })
        })
        .collect()
}

/// Removes `#[callback_data(...)]` attributes from every field of `input`.
/// Called after we've already read what we need from them (currently just
/// the `owner` marker) — they must not survive into the re-emitted struct,
/// since `callback_data` is also the name of this very attribute macro.
fn strip_callback_data_field_attrs(input: &mut DeriveInput) {
    if let Data::Struct(data_struct) = &mut input.data {
        if let Fields::Named(named) = &mut data_struct.fields {
            for field in named.named.iter_mut() {
                field.attrs.retain(|a| !a.path().is_ident("callback_data"));
            }
        }
    }
}

fn is_owner_attr(attr: &syn::Attribute) -> bool {
    if !attr.path().is_ident("callback_data") {
        return false;
    }
    let mut found = false;
    let _ = attr.parse_nested_meta(|meta| {
        if meta.path.is_ident(OWNER_ATTR) {
            found = true;
        }
        Ok(())
    });
    found
}

/// Generates `impl FlatField for #name` where each named field is packed
/// in declaration order via its own `FlatField` impl (this is how nesting
/// "just works" — a field whose type also derives `FlatField` flattens
/// straight into the parent's token stream).
fn flat_field_impl_for_struct(name: &Ident, fields: &[FieldInfo]) -> TokenStream2 {
    let field_idents: Vec<&Ident> = fields.iter().map(|f| &f.ident).collect();
    let field_types: Vec<&Type> = fields.iter().map(|f| &f.ty).collect();

    let token_count = if field_types.is_empty() {
        quote! { 0 }
    } else {
        quote! { #( <#field_types as callback_data::FlatField>::TOKEN_COUNT )+* }
    };

    let pack_stmts = field_idents.iter().map(|ident| {
        quote! { callback_data::FlatField::pack_into(&self.#ident, out)?; }
    });

    let unpack_stmts = field_idents
        .iter()
        .zip(field_types.iter())
        .map(|(ident, ty)| {
            quote! {
                let #ident = <#ty as callback_data::FlatField>::unpack_from(tokens)?;
            }
        });

    quote! {
        impl callback_data::FlatField for #name {
            const TOKEN_COUNT: usize = #token_count;

            fn pack_into(&self, out: &mut ::std::vec::Vec<::std::string::String>) -> ::std::result::Result<(), callback_data::PackError> {
                #( #pack_stmts )*
                Ok(())
            }

            fn unpack_from(
                tokens: &mut ::std::vec::IntoIter<::std::string::String>,
            ) -> ::std::result::Result<Self, callback_data::UnpackError> {
                #( #unpack_stmts )*
                Ok(Self { #( #field_idents ),* })
            }
        }
    }
}

/// Body of `owner_id()` for the explicit `CallbackDataTrait` impl: `None`
/// if no field is marked `#[callback_data(owner)]`, `Some(self.field)` if
/// exactly one is (the field's type must be `i64` — a mismatch here is a
/// plain, readable type-mismatch compile error). More than one marked
/// field is a hard compile error, since "owner" is meant to be
/// unambiguous.
fn owner_id_body(name: &Ident, fields: &[FieldInfo]) -> TokenStream2 {
    let owners: Vec<&FieldInfo> = fields.iter().filter(|f| f.is_owner).collect();

    match owners.as_slice() {
        [] => quote! { ::std::option::Option::None },
        [owner] => {
            let ident = &owner.ident;
            quote! { ::std::option::Option::Some(self.#ident) }
        }
        _ => {
            let msg = format!(
                "at most one field on `{name}` may be marked #[callback_data(owner)], found {}",
                owners.len()
            );
            syn::Error::new_spanned(name, msg).to_compile_error()
        }
    }
}

// ---------------------------------------------------------------------
// enums: variant index token + padded, flattened variant fields
// ---------------------------------------------------------------------

struct VariantInfo {
    ident: Ident,
    field_idents: Vec<Ident>,
    field_types: Vec<Type>,
    is_unit: bool,
}

fn flat_field_impl_for_enum(name: &Ident, data_enum: &syn::DataEnum) -> TokenStream2 {
    let mut variants = Vec::new();
    for variant in &data_enum.variants {
        let (field_idents, field_types, is_unit) = match &variant.fields {
            Fields::Named(named) => {
                let idents = named
                    .named
                    .iter()
                    .map(|f| f.ident.clone().unwrap())
                    .collect();
                let types = named.named.iter().map(|f| f.ty.clone()).collect();
                (idents, types, false)
            }
            Fields::Unit => (Vec::new(), Vec::new(), true),
            Fields::Unnamed(_) => {
                return syn::Error::new_spanned(
                    &variant.ident,
                    "FlatField enums must use unit variants or named-field variants (no tuple variants) — this keeps generated field names available for nested #[callback_data(owner)]-style access",
                )
                .to_compile_error();
            }
        };
        variants.push(VariantInfo {
            ident: variant.ident.clone(),
            field_idents,
            field_types,
            is_unit,
        });
    }

    if variants.is_empty() {
        return syn::Error::new_spanned(name, "FlatField cannot be derived for an empty enum")
            .to_compile_error();
    }

    // Per-variant token count, computed at macro-expansion time is NOT
    // possible for generic-in-nested-type cases without evaluating const
    // exprs, so we emit `<Ty as FlatField>::TOKEN_COUNT` additions and let
    // rustc const-evaluate the max via a const fn — see MAX_TOKENS below.
    let variant_token_count_exprs: Vec<TokenStream2> = variants
        .iter()
        .map(|v| {
            if v.field_types.is_empty() {
                quote! { 0usize }
            } else {
                let tys = &v.field_types;
                quote! { #( <#tys as callback_data::FlatField>::TOKEN_COUNT )+* }
            }
        })
        .collect();

    let max_ident = format_ident!("__{}_MAX_VARIANT_TOKENS", name);

    let pack_arms = variants.iter().enumerate().map(|(idx, v)| {
        let idx = idx as u32;
        let variant_ident = &v.ident;
        let field_idents = &v.field_idents;

        let pattern = if v.is_unit {
            quote! { Self::#variant_ident }
        } else {
            quote! { Self::#variant_ident { #( #field_idents ),* } }
        };

        let field_pack_stmts = field_idents.iter().map(|ident| {
            quote! { callback_data::FlatField::pack_into(#ident, out)?; }
        });

        let this_variant_count = &variant_token_count_exprs[idx as usize];

        quote! {
            #pattern => {
                out.push(#idx.to_string());
                #( #field_pack_stmts )*
                let __written = #this_variant_count;
                for _ in __written..#max_ident {
                    out.push(::std::string::String::new());
                }
            }
        }
    });

    let unpack_arms = variants.iter().enumerate().map(|(idx, v)| {
        let idx = idx as u32;
        let variant_ident = &v.ident;
        let field_idents = &v.field_idents;
        let field_types = &v.field_types;
        let this_variant_count = &variant_token_count_exprs[idx as usize];

        let field_unpack_stmts = field_idents
            .iter()
            .zip(field_types.iter())
            .map(|(ident, ty)| {
                quote! {
                    let #ident = <#ty as callback_data::FlatField>::unpack_from(tokens)?;
                }
            });

        let construct = if v.is_unit {
            quote! { Self::#variant_ident }
        } else {
            quote! { Self::#variant_ident { #( #field_idents ),* } }
        };

        quote! {
            #idx => {
                #( #field_unpack_stmts )*
                let __consumed = #this_variant_count;
                for _ in __consumed..#max_ident {
                    let _ = tokens.next().ok_or(callback_data::UnpackError::NotEnoughTokens {
                        field: "<enum variant padding>",
                    })?;
                }
                ::std::result::Result::Ok(#construct)
            }
        }
    });

    let type_name_str = name.to_string();
    let variant_count = variants.len() as u32;
    let _ = variant_count; // reserved for future exhaustiveness diagnostics

    quote! {
        #[allow(non_upper_case_globals)]
        const #max_ident: usize = {
            let counts: &[usize] = &[ #( #variant_token_count_exprs ),* ];
            let mut max = 0usize;
            let mut i = 0usize;
            while i < counts.len() {
                if counts[i] > max { max = counts[i]; }
                i += 1;
            }
            max
        };

        impl callback_data::FlatField for #name {
            const TOKEN_COUNT: usize = 1 + #max_ident;

            fn pack_into(&self, out: &mut ::std::vec::Vec<::std::string::String>) -> ::std::result::Result<(), callback_data::PackError> {
                match self {
                    #( #pack_arms )*
                }
                Ok(())
            }

            fn unpack_from(
                tokens: &mut ::std::vec::IntoIter<::std::string::String>,
            ) -> ::std::result::Result<Self, callback_data::UnpackError> {
                let __index_token = tokens.next().ok_or(callback_data::UnpackError::NotEnoughTokens {
                    field: "<enum variant index>",
                })?;
                let __index: u32 = __index_token.parse().map_err(|_| callback_data::UnpackError::UnknownVariant {
                    index: u32::MAX,
                    ty: #type_name_str,
                })?;
                match __index {
                    #( #unpack_arms )*
                    other => ::std::result::Result::Err(callback_data::UnpackError::UnknownVariant {
                        index: other,
                        ty: #type_name_str,
                    }),
                }
            }
        }
    }
}
