// Copyright 2022 Google LLC
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

use std::collections::HashSet;

use proc_macro2::{Ident, TokenStream};
use syn::{
    parenthesized,
    parse::{Parse, Parser},
    Attribute, LitStr,
};

use crate::conversion::{
    api::{CppVisibility, Layout, Virtualness},
    convert_error::{ConvertErrorWithContext, ErrorContext},
    ConvertError,
};

/// The set of all annotations that autocxx_bindgen has added
/// for our benefit.
#[derive(Debug)]
pub(crate) struct AutocxxBindgenAnnotations(Vec<AutocxxBindgenAttribute>);

impl AutocxxBindgenAnnotations {
    /// Interprets any `autocxx::bindgen_annotation` within this item's
    /// attributes.
    pub(crate) fn new(attrs: &[Attribute]) -> Self {
        let s = Self(
            attrs
                .iter()
                .filter_map(|attr| {
                    if attr.path.segments.last().unwrap().ident == "bindgen_annotation" {
                        let r: Result<AutocxxBindgenAttribute, syn::Error> = attr.parse_args();
                        r.ok()
                    } else {
                        None
                    }
                })
                .collect(),
        );
        s
    }

    /// Whether the given attribute is present.
    pub(super) fn has_attr(&self, attr_name: &str) -> bool {
        self.0.iter().any(|a| a.is_ident(attr_name))
    }

    /// The C++ visibility of the item.
    pub(super) fn get_cpp_visibility(&self) -> CppVisibility {
        if self.has_attr("visibility_private") {
            CppVisibility::Private
        } else if self.has_attr("visibility_protected") {
            CppVisibility::Protected
        } else {
            CppVisibility::Public
        }
    }

    /// Whether the item is virtual.
    pub(super) fn get_virtualness(&self) -> Virtualness {
        if self.has_attr("pure_virtual") {
            Virtualness::PureVirtual
        } else if self.has_attr("bindgen_virtual") {
            Virtualness::Virtual
        } else {
            Virtualness::None
        }
    }

    fn parse_if_present<T: Parse>(&self, annotation: &str) -> Option<T> {
        self.0
            .iter()
            .find(|a| a.is_ident(annotation))
            .map(|a| a.parse_args().unwrap())
    }

    fn string_if_present(&self, annotation: &str) -> Option<String> {
        let ls: Option<LitStr> = self.parse_if_present(annotation);
        ls.map(|ls| ls.value())
    }

    /// The in-memory layout of the item.
    pub(super) fn get_layout(&self) -> Option<Layout> {
        self.parse_if_present("layout")
    }

    /// The original C++ name, which bindgen may have changed.
    pub(super) fn get_original_name(&self) -> Option<String> {
        self.string_if_present("original_name")
    }

    fn get_bindgen_special_member_annotation(&self) -> Option<String> {
        self.string_if_present("special_member")
    }

    /// Whether this is a move constructor.
    pub(super) fn is_move_constructor(&self) -> bool {
        self.get_bindgen_special_member_annotation()
            .map_or(false, |val| val == "move_ctor")
    }

    /// Any reference parameters or return values.
    pub(super) fn get_reference_parameters_and_return(&self) -> (HashSet<Ident>, bool) {
        let mut ref_params = HashSet::new();
        let mut ref_return = false;
        for a in &self.0 {
            if a.is_ident("ret_type_reference") {
                ref_return = true;
            } else if a.is_ident("arg_type_reference") {
                let r: Result<Ident, syn::Error> = a.parse_args();
                if let Ok(ls) = r {
                    ref_params.insert(ls);
                }
            }
        }
        (ref_params, ref_return)
    }

    // Remove `bindgen_` attributes. They don't have a corresponding macro defined anywhere,
    // so they will cause compilation errors if we leave them in.
    // We may return an error if one of the bindgen attributes shows that the
    // item can't be processed.
    pub(crate) fn remove_bindgen_attrs(
        attrs: &mut Vec<Attribute>,
        id: Ident,
    ) -> Result<(), ConvertErrorWithContext> {
        let annotations = Self::new(&attrs);
        if annotations.has_attr("unused_template_param") {
            return Err(ConvertErrorWithContext(
                ConvertError::UnusedTemplateParam,
                Some(ErrorContext::Item(id)),
            ));
        }
        attrs.retain(|a| !(a.path.segments.last().unwrap().ident == "bindgen_annotation"));
        Ok(())
    }
}

#[derive(Debug)]
struct AutocxxBindgenAttribute {
    annotation_name: Ident,
    body: Option<TokenStream>,
}

impl AutocxxBindgenAttribute {
    fn is_ident(&self, name: &str) -> bool {
        self.annotation_name == name
    }

    fn parse_args<T: Parse>(&self) -> Result<T, syn::Error> {
        T::parse.parse2(self.body.as_ref().unwrap().clone())
    }
}

impl Parse for AutocxxBindgenAttribute {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let annotation_name: Ident = input.parse()?;
        if input.peek(syn::token::Paren) {
            let body_contents;
            parenthesized!(body_contents in input);
            Ok(Self {
                annotation_name,
                body: Some(body_contents.parse()?),
            })
        } else if !input.is_empty() {
            Err(input.error("expected nothing"))
        } else {
            Ok(Self {
                annotation_name,
                body: None,
            })
        }
    }
}
