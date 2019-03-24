extern crate proc_macro as pm1;

use proc_macro2::{Ident, Literal, Punct, Spacing, TokenStream, TokenTree};
use quote::{quote, quote_spanned, ToTokens};
use std::collections::HashMap;
use std::iter::FromIterator;
use syn::spanned::Spanned;
use syn::{
    parse_macro_input, parse_quote, Attribute, FnArg, ImplItem, ItemImpl, Lit, LitStr, Meta,
    NestedMeta, Type,
};

#[proc_macro_attribute]
pub fn rpc(_attr: pm1::TokenStream, item: pm1::TokenStream) -> pm1::TokenStream {
    item
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum AttrKey {
    Name,
    Notification,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum AttrValue {
    String(String),
    Presence,
}

fn parse_rpc_attr(attr: &Attribute) -> Vec<(AttrKey, AttrValue)> {
    let meta = match attr.parse_meta() {
        Ok(m) => m,
        Err(_) => return Vec::new(),
    };

    if &format!("{}", meta.name()) != "rpc" {
        return Vec::new();
    }

    match meta {
        Meta::List(list) => list
            .nested
            .iter()
            .flat_map(|met| match met {
                NestedMeta::Meta(m) => parse_rpc_attr_meta(m),
                _ => Vec::new(),
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn parse_rpc_attr_meta(meta: &Meta) -> Vec<(AttrKey, AttrValue)> {
    match meta {
        Meta::Word(ident) => {
            let value: &str = &format!("{}", ident);
            match value {
                "notification" => vec![(AttrKey::Notification, AttrValue::Presence)],
                _ => Vec::new(),
            }
        }
        Meta::NameValue(pair) => {
            let name: &str = &format!("{}", pair.ident);
            let value = &pair.lit;
            match (name, value) {
                ("name", Lit::Str(s)) => vec![(AttrKey::Name, AttrValue::String(s.value()))],
                _ => Vec::new(),
            }
        }
        _ => Vec::new(),
    }
}

#[proc_macro]
pub fn rpc_impl_struct(input: pm1::TokenStream) -> pm1::TokenStream {
    let mut in_impl = parse_macro_input!(input as ItemImpl);

    // extract methods
    let methods = in_impl.items.iter().filter_map(|item| match item {
        ImplItem::Method(m) => Some(m),
        _ => None,
    });

    // generate delegated entries for methods we care about
    let delegates = methods.map(|method| {
        let attrs: HashMap<AttrKey, AttrValue> = HashMap::from_iter(method.attrs.iter().flat_map(parse_rpc_attr));

        let funname = &method.sig.ident;
        let funnames = Lit::Str(LitStr::new(&format!("{}", funname), method.sig.ident.span()));

        let name = if let Some(AttrValue::String(namestr)) = attrs.get(&AttrKey::Name) {
            namestr.clone()
        } else {
            format!("{}", method.sig.ident)
        };

        let name = Lit::Str(LitStr::new(&name, method.sig.ident.span()));

        let output = &method.sig.decl.output;
        let inputs = &method.sig.decl.inputs;

        let types: Vec<&Type> = inputs
            .iter()
            .filter_map(|input| {
                if let FnArg::Captured(arg) = input {
                    Some(&arg.ty)
                } else {
                    None
                }
            })
            .collect();

        let args = (0..types.len()).map(|n| {
            let mut stream = TokenStream::new();
            let tree: Vec<TokenTree> = vec![
                Ident::new("args", method.span()).into(),
                Punct::new('.', Spacing::Alone).into(),
                Literal::usize_unsuffixed(n).into(),
            ];
            stream.extend(tree);
            stream
        });

        let typdef = types.clone();
        let fundef = quote! {&(Self::#funname as fn(&_, #(#typdef),*) #output) };

        let (param_parser, kind) = if let Some(AttrValue::Presence) = attrs.get(&AttrKey::Notification) {
            (quote_spanned! {method.span()=>
                match ::rpc_macro_support::parse_params(params) {
                    Ok(p) => p,
                    Err(err) => {
                        ::log::error!("wrong parameter types for notification (rpc: {}), skip", #name);
                        return;
                    }
                }
            }, Lit::Str(LitStr::new("notification", method.span())))
        } else {
            (quote_spanned! {method.span()=>
                ::rpc_macro_support::parse_params(params)?
            }, Lit::Str(LitStr::new("method", method.span())))
        };

        let fun = if types.is_empty() {
            quote_spanned! {method.span()=>
                ::log::debug!("receiving for typed {} {} (rpc: {}): no params", #kind, #funnames, #name);
                let fun = #fundef;
                ::log::debug!("handling typed {} {} (rpc: {})", #kind, #funnames, #name);
                fun(base)
            }
        } else if types.len() == 1 {
            let typdsc = types.clone();
            quote_spanned! {method.span()=>
                ::log::debug!("receiving for typed {} {} (rpc: {}): parsing params to ({})", #kind, #funnames, #name, stringify!(#(#typdsc),*));
                let arg: #(#types),* = #param_parser;
                let fun = #fundef;
                ::log::debug!("handling typed {} {} (rpc: {})", #kind, #funnames, #name);
                fun(base, arg)
            }
        } else {
            let typdsc = types.clone();
            quote_spanned! {method.span()=>
                ::log::debug!("receiving for typed {} {} (rpc: {}): parsing params to ({})", #kind, #funnames, #name, stringify!(#(#typdsc),*));
                let args: (#(#types),*) = #param_parser;
                let fun = #fundef;
                ::log::debug!("handling typed {} {} (rpc: {})", #kind, #funnames, #name);
                fun(base, #(#args),*)
            }
        };

        Some(if let Some(AttrValue::Presence) = attrs.get(&AttrKey::Notification) {
            quote_spanned! {method.span()=>
                #[allow(unused_variables, unused_must_use)]
                del.add_notification(#name, move |base, params| {
                    #fun;
                });
            }
        } else {
            quote_spanned! {method.span()=>
                #[allow(unused_variables)]
                del.add_method(#name, move |base, params| {
                    #fun.map(|res| ::serde_json::to_value(&res).unwrap())
                });
            }
        })
    });

    // create a new method for delegates
    let delegate: ImplItem = parse_quote! {
        /// Transform this into an `IoDelegate`, automatically wrapping the parameters.
        fn to_delegate<M: ::jsonrpc_core::Metadata>(self) -> ::jsonrpc_macros::IoDelegate<Self, M> {
            let mut del = ::jsonrpc_macros::IoDelegate::new(self.into());
            #(#delegates)*
            del
        }
    };

    // insert method
    in_impl.items.push(delegate);

    // rebuild token stream
    in_impl.into_token_stream().into()
}
