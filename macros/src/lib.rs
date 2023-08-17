use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use std::collections::HashMap;
use syn::{parse_macro_input, Data, DeriveInput, Expr, Lit, Meta, MetaList, NestedMeta, Variant};

#[proc_macro_derive(TowerTypeData, attributes(tower, prerequisite, capacity, generate))]
pub fn derive_tower_type_data(input: TokenStream) -> TokenStream {
    let DeriveInput {
        ident, data, attrs, ..
    } = parse_macro_input!(input);
    if let Data::Enum(enum_data) = data {
        let mut tower_delays = HashMap::<String, proc_macro2::TokenStream>::new();
        let mut tower_prerequisites = Vec::<proc_macro2::TokenStream>::new();
        let mut tower_capacities = Vec::<proc_macro2::TokenStream>::new();
        let mut tower_generations = Vec::<proc_macro2::TokenStream>::new();
        let mut score_weights = Vec::<proc_macro2::TokenStream>::new();
        let mut spawnables = Vec::<proc_macro2::TokenStream>::new();
        let mut sensor_radii = Vec::<proc_macro2::TokenStream>::new();
        let mut downgrades = Vec::<proc_macro2::TokenStream>::new();

        for Variant {
            ident: variant,
            attrs: variant_attrs,
            fields,
            ..
        } in enum_data.variants
        {
            assert!(fields.is_empty());

            let mut prerequisites = Vec::<proc_macro2::TokenStream>::new();
            let mut unit_capacities = HashMap::<_, proc_macro2::TokenStream>::new();
            let mut unit_generations = HashMap::<_, proc_macro2::TokenStream>::new();
            let mut sensor_radius = None;
            let mut score_weight = None;

            for attribute in attrs.iter().chain(&variant_attrs) {
                let meta = attribute.parse_meta().expect("couldn't parse as meta");

                if attribute.path.is_ident("tower") {
                    if let Meta::List(MetaList { nested, .. }) = meta {
                        for meta in nested {
                            match meta {
                                NestedMeta::Meta(Meta::NameValue(meta)) => {
                                    let name = meta.path;
                                    let value = meta.lit;
                                    if name.is_ident("sensor_radius") {
                                        sensor_radius = Some(quote! {
                                            Self::#variant => #value
                                        });
                                    } else if name.is_ident("score_weight") {
                                        score_weight = Some(quote! {
                                            Self::#variant => #value
                                        });
                                    } else {
                                        panic!("unrecognized {:?}", name.to_token_stream());
                                    }
                                }
                                NestedMeta::Meta(Meta::Path(path)) => {
                                    if path.is_ident("spawnable") {
                                        spawnables.push(quote! {
                                            Self::#variant => true
                                        });
                                    } else {
                                        panic!("unrecognized {:?}", path.to_token_stream());
                                    }
                                }
                                _ => panic!("expected meta"),
                            }
                        }
                    } else {
                        panic!("expected list");
                    }
                } else if attribute.path.is_ident("prerequisite") {
                    if let Meta::List(MetaList { nested, .. }) = meta {
                        for meta in nested {
                            match meta {
                                NestedMeta::Lit(Lit::Int(int)) => {
                                    let seconds: u16 = int.base10_parse().unwrap();
                                    tower_delays.insert(
                                        format!("{:?}", variant),
                                        quote! {
                                            Self::#variant => Ticks::from_whole_secs(#seconds)
                                        },
                                    );
                                }
                                NestedMeta::Meta(Meta::Path(path)) => {
                                    downgrades.push(quote! {
                                        Self::#variant => Self::#path
                                    });
                                }
                                NestedMeta::Meta(Meta::NameValue(meta)) => {
                                    let path = meta.path;
                                    let count = meta.lit;
                                    prerequisites.push(quote! {
                                        Self::#path => (#count)
                                    });
                                }
                                _ => panic!("expected path"),
                            }
                        }
                    } else {
                        panic!("expected list");
                    }
                } else if attribute.path.is_ident("capacity") {
                    if let Meta::List(MetaList { nested, .. }) = meta {
                        for meta in nested {
                            match meta {
                                NestedMeta::Meta(Meta::NameValue(meta)) => {
                                    let key = meta.path.clone();
                                    let name = meta.path;
                                    let value = meta.lit;
                                    unit_capacities.insert(
                                        Some(key),
                                        quote! {
                                            Unit::#name => #value
                                        },
                                    );
                                }
                                _ => panic!("expected meta"),
                            }
                        }
                    } else {
                        panic!("expected list");
                    }
                } else if attribute.path.is_ident("generate") {
                    if let Meta::List(MetaList { nested, .. }) = meta {
                        for meta in nested {
                            match meta {
                                NestedMeta::Meta(Meta::NameValue(meta)) => {
                                    let key = meta.path.clone();
                                    let name = meta.path;
                                    let value = meta.lit;
                                    unit_generations.insert(
                                        key,
                                        quote! {
                                            Unit::#name => Some(Ticks::from_whole_secs(#value))
                                        },
                                    );
                                }
                                _ => panic!("expected meta"),
                            }
                        }
                    } else {
                        panic!("expected list");
                    }
                }
            }

            tower_prerequisites.push(quote! {
                Self::#variant => match tower_type {
                    #(#prerequisites,)*
                    _ => 0
                }
            });

            let unit_capacities = unit_capacities.into_values();

            tower_capacities.push(quote! {
                Self::#variant => match unit {
                    #(#unit_capacities,)*
                    _ => 0
                }
            });

            let unit_generations = unit_generations.into_values();

            tower_generations.push(quote! {
                Self::#variant => match unit {
                    #(#unit_generations,)*
                    _ => None
                }
            });

            if let Some(sensor_radius) = sensor_radius {
                sensor_radii.push(sensor_radius);
            }

            if let Some(score_weight) = score_weight {
                score_weights.push(score_weight);
            }
        }

        let tower_delays = tower_delays.into_values();

        let output: proc_macro2::TokenStream = quote! {
            impl #ident {
                /// Doesn't count ruler boost.
                pub fn raw_unit_capacity(self, unit: Unit) -> usize {
                    match self {
                        #(#tower_capacities,)*
                    }
                }

                pub fn unit_generation(self, unit: Unit) -> Option<Ticks> {
                    match self {
                        #(#tower_generations,)*
                    }
                }

                /// Upgrading to this tower requires this much of this other tower.
                pub fn prerequisite(self, tower_type: TowerType) -> u8 {
                    match self {
                        #(#tower_prerequisites,)*
                        _ => 0
                    }
                }

                /// How long it should take to upgrade/downgrade to this tower.
                pub fn delay(self) -> Ticks {
                    match self {
                        #(#tower_delays,)*
                        _ => Ticks::ZERO
                    }
                }

                /// Which tower this tower can downgrade to, if any.
                pub fn downgrade(self) -> Option<Self> {
                    Some(match self {
                        #(#downgrades,)*
                        _ => return None
                    })
                }

                pub fn sensor_radius(self) -> u16 {
                    match self {
                        #(#sensor_radii,)*
                    }
                }

                pub fn score_weight(self) -> u32 {
                    match self {
                        #(#score_weights,)*
                        _ => 1
                    }
                }

                pub fn is_spawnable(self) -> bool {
                    match self {
                        #(#spawnables,)*
                        _ => false
                    }
                }
            }
        };
        output.into()
    } else {
        panic!("expected an enum")
    }
}

#[allow(unused)]
fn str_lit_to_expr(lit: Lit) -> Expr {
    if let Lit::Str(s) = lit {
        let string = s.value();
        let str = string.as_str();
        let ret = syn::parse_str::<Expr>(str).expect(str);
        ret
    } else {
        panic!("expected string literal")
    }
}
