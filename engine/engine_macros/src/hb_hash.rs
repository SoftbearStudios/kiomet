// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use proc_macro::TokenStream;
use proc_macro2::{Ident, Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{
    parse_macro_input, parse_quote, Data, DataEnum, DataStruct, DeriveInput, Field, Fields,
    GenericParam, Generics,
};

/// Honey Badger Hash (doesn't care about restrictions against hashing floats)
pub(crate) fn derive_hb_hash(input: TokenStream) -> TokenStream {
    let DeriveInput {
        ident,
        data,
        generics,
        ..
    } = parse_macro_input!(input);

    fn destructure_fields(fields: &Fields) -> TokenStream2 {
        match fields {
            Fields::Named(named) => {
                let names = named.named.iter().map(|field| &field.ident);
                quote! {
                    {#(#names),*}
                }
            }
            Fields::Unnamed(unnamed) => {
                let names = (0..unnamed.unnamed.iter().count())
                    .map(|i| Ident::new(&format!("f{i}"), Span::mixed_site()));
                quote! {
                    (#(#names),*)
                }
            }
            Fields::Unit => {
                quote! {}
            }
        }
        .into()
    }

    fn hash_fields(fields: &Fields) -> impl Iterator<Item = TokenStream2> + '_ {
        fields.iter().enumerate().map(
            |(
                i,
                Field {
                    ty, attrs, ident, ..
                },
            )| {
                let tmp = ident
                    .clone()
                    .unwrap_or_else(|| Ident::new(&format!("f{i}"), Span::mixed_site()));
                let trayt = if attrs.iter().any(|attr| {
                    attr.parse_meta()
                        .ok()
                        .map(|meta| meta.path().is_ident("hb_hash"))
                        .unwrap_or(false)
                }) {
                    quote!(common_util::hash::HbHash)
                } else {
                    quote!(std::hash::Hash)
                };
                quote! {
                    <#ty as #trayt>::hash(#tmp, state);
                }
                .into()
            },
        )
    }

    let output = match data {
        Data::Struct(DataStruct { fields, .. }) => {
            let destructure_fields = destructure_fields(&fields);
            let hash_fields = hash_fields(&fields);

            let output = quote! {
                let Self #destructure_fields = self;
                #(#hash_fields)*
            };
            output.into()
        }
        Data::Enum(DataEnum { variants, .. }) => {
            let hash_variants = variants.into_iter().enumerate().map(|(i, variant)| {
                let ident = variant.ident;
                let destructure_fields = destructure_fields(&variant.fields);
                let hash_fields = hash_fields(&variant.fields);
                quote! {
                    Self::#ident #destructure_fields => {
                        #i.hash(state);
                        #(#hash_fields)*
                    }
                }
            });

            let output = quote! {
                match self {
                    #(#hash_variants),*
                }
            };
            output
        }
        Data::Union(_) => panic!("unions not supported"),
    };

    let generics = add_trait_bounds(generics);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    quote! {
        impl #impl_generics std::hash::Hash for #ident #ty_generics #where_clause {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                #output
            }
        }
    }
    .into()
}

// Add a bound `T: Vertex` to every type parameter T.
fn add_trait_bounds(mut generics: Generics) -> Generics {
    for param in &mut generics.params {
        if let GenericParam::Type(ref mut type_param) = *param {
            type_param.bounds.push(parse_quote!(Vertex));
        }
    }
    generics
}
