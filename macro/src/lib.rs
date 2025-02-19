// Copyright 2020 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//    https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use autocxx_parser::{IncludeCpp, SubclassAttrs};
use proc_macro::TokenStream;
use proc_macro2::{Ident, Span};
use proc_macro_error::{abort, proc_macro_error};
use quote::quote;
use syn::parse::Parser;
use syn::{parse_macro_input, parse_quote, Fields, Item, ItemStruct, Visibility};

/// Implementation of the `include_cpp` macro. See documentation for `autocxx` crate.
#[proc_macro_error]
#[proc_macro]
pub fn include_cpp_impl(input: TokenStream) -> TokenStream {
    let include_cpp = parse_macro_input!(input as IncludeCpp);
    TokenStream::from(include_cpp.generate_rs())
}

/// Attribute to state that a Rust `struct` is a C++ subclass.
/// This adds an additional field to the struct which autocxx uses to
/// track a C++ instantiation of this Rust subclass.
#[proc_macro_error]
#[proc_macro_attribute]
pub fn subclass(attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut s: ItemStruct =
        syn::parse(item).unwrap_or_else(|_| abort!(Span::call_site(), "Expected a struct"));
    if !matches!(s.vis, Visibility::Public(..)) {
        use syn::spanned::Spanned;
        abort!(s.vis.span(), "Rust subclasses of C++ types must by public");
    }
    let id = &s.ident;
    let cpp_ident = Ident::new(&format!("{}Cpp", id), Span::call_site());
    let input = quote! {
        cpp_peer: autocxx::subclass::CppSubclassCppPeerHolder<ffi:: #cpp_ident>
    };
    let parser = syn::Field::parse_named;
    let new_field = parser.parse2(input).unwrap();
    s.fields = match &mut s.fields {
        Fields::Named(fields) => {
            fields.named.push(new_field);
            s.fields
        },
        Fields::Unit => Fields::Named(parse_quote! {
            {
                #new_field
            }
        }),
        _ => abort!(Span::call_site(), "Expect a struct with named fields - use struct A{} or struct A; as opposed to struct A()"),
    };
    let subclass_attrs: SubclassAttrs = syn::parse(attr)
        .unwrap_or_else(|_| abort!(Span::call_site(), "Unable to parse attributes"));
    let self_owned_bit = if subclass_attrs.self_owned {
        Some(quote! {
            impl autocxx::subclass::CppSubclassSelfOwned<ffi::#cpp_ident> for #id {}
        })
    } else {
        None
    };
    let toks = quote! {
        #s

        impl autocxx::subclass::CppSubclass<ffi::#cpp_ident> for #id {
            fn peer_holder_mut(&mut self) -> &mut autocxx::subclass::CppSubclassCppPeerHolder<ffi::#cpp_ident> {
                &mut self.cpp_peer
            }
            fn peer_holder(&self) -> &autocxx::subclass::CppSubclassCppPeerHolder<ffi::#cpp_ident> {
                &self.cpp_peer
            }
        }

        #self_owned_bit
    };
    toks.into()
}

/// Attribute to state that a Rust type is to be exported to C++
/// in the `extern "Rust"` section of the generated `cxx` bindings.
#[proc_macro_error]
#[proc_macro_attribute]
pub fn extern_rust_type(attr: TokenStream, input: TokenStream) -> TokenStream {
    if !attr.is_empty() {
        abort!(Span::call_site(), "Expected no attributes");
    }
    let i: Item =
        syn::parse(input.clone()).unwrap_or_else(|_| abort!(Span::call_site(), "Expected an item"));
    match i {
        Item::Struct(..) | Item::Enum(..) | Item::Fn(..) => {}
        _ => abort!(Span::call_site(), "Expected a struct or enum"),
    }
    input
}

/// Attribute to state that a Rust function is to be exported to C++
/// in the `extern "Rust"` section of the generated `cxx` bindings.
#[proc_macro_error]
#[proc_macro_attribute]
pub fn extern_rust_function(attr: TokenStream, input: TokenStream) -> TokenStream {
    if !attr.is_empty() {
        abort!(Span::call_site(), "Expected no attributes");
    }
    let i: Item =
        syn::parse(input.clone()).unwrap_or_else(|_| abort!(Span::call_site(), "Expected an item"));
    match i {
        Item::Fn(..) => {}
        _ => abort!(Span::call_site(), "Expected a function"),
    }
    input
}
