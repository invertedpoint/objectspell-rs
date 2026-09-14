//! The three attribute macros behind `objectspell`.
//!
//! Use them through the `objectspell` crate — `#[objectspell::emitter]`,
//! `#[objectspell::state]`, `#[objectspell::receiver]` — which re-exports all three and is the
//! crate their generated code names. Depending on this crate directly will not work.
//!
//! See the [`objectspell`](https://docs.rs/objectspell) documentation for what they do.

#![warn(missing_docs)]

extern crate proc_macro;

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream, Result};
use syn::{parse_macro_input, ImplItem, ItemImpl, ItemStruct, Signature, Token, Visibility};

/// The name a parameter travels under, which is how an emitter's argument finds the handler
/// parameter it belongs to.
///
/// Leading underscores are dropped. In Rust they only silence the unused-variable lint, so a
/// handler that ignores an argument still declares the same parameter as the signal that
/// carries it — `_message` and `message` are one name, not two.
fn wire_name(ident: &syn::Ident) -> String {
    ident.to_string().trim_start_matches('_').to_string()
}

// ============================================================================
// emitter
// ============================================================================

struct EmitterMethod {
    attrs: Vec<syn::Attribute>,
    vis: Visibility,
    sig: Signature,
}

impl Parse for EmitterMethod {
    fn parse(input: ParseStream) -> Result<Self> {
        let attrs = input.call(syn::Attribute::parse_outer)?;
        let vis: Visibility = input.parse()?;
        let sig: Signature = input.parse()?;
        input.parse::<Token![;]>()?;
        Ok(EmitterMethod { attrs, vis, sig })
    }
}

struct EmitterBlock {
    attrs: Vec<syn::Attribute>,
    _vis: Visibility,
    _trait_token: Token![trait],
    ident: syn::Ident,
    methods: Vec<EmitterMethod>,
}

impl Parse for EmitterBlock {
    fn parse(input: ParseStream) -> Result<Self> {
        let attrs = input.call(syn::Attribute::parse_outer)?;
        let _vis: Visibility = input.parse()?;
        let _trait_token: Token![trait] = input.parse()?;
        let ident: syn::Ident = input.parse()?;

        let content;
        syn::braced!(content in input);

        let mut methods = Vec::new();
        while !content.is_empty() {
            methods.push(content.parse()?);
        }

        Ok(EmitterBlock {
            attrs,
            _vis,
            _trait_token,
            ident,
            methods,
        })
    }
}

/// Whether the user already wrote a doc comment, so the macro should not add one of its own.
fn has_doc(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| attr.path().is_ident("doc"))
}

/// The `#[objectspell::emitter]` macro parses a `trait StructName { ... }` definition.
/// It drops the trait completely and instead generates an inherent `impl StructName` block
/// containing async methods that broadcast on the internal `emitter_core`.
#[proc_macro_attribute]
pub fn emitter(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let block = parse_macro_input!(item as EmitterBlock);
    let name = &block.ident;

    let mut generated_methods = Vec::new();
    let mut route_names = Vec::new();

    for method in block.methods {
        // A trait method has no visibility of its own — it is as visible as the trait. So an
        // undecorated declaration means "as public as the component", not "private"; a private
        // sender would be unreachable from anywhere but its own module. An explicit visibility
        // is still honoured, so `pub(crate)` narrows it on purpose.
        let vis = match &method.vis {
            Visibility::Inherited => quote! { pub },
            explicit => quote! { #explicit },
        };
        let sig = &method.sig;
        let sig_name = &sig.ident;
        let sig_name_str = sig_name.to_string();
        route_names.push(sig_name_str.clone());

        let mut with_params = Vec::new();
        let mut fn_args = Vec::new();
        fn_args.push(quote! { &self });

        for arg in &sig.inputs {
            if let syn::FnArg::Typed(pat_type) = arg {
                if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                    let p_name = &pat_ident.ident;
                    let p_name_str = wire_name(p_name);
                    let ty = &pat_type.ty;
                    fn_args.push(quote! { #p_name: #ty });
                    with_params.push(quote! { .with_param(#p_name_str, #p_name) });
                }
            }
        }

        // Whatever the user wrote on the declaration travels to the generated method — the
        // declaration is discarded, so this is the only place their doc comment can land. A
        // generated method with no doc at all would trip `missing_docs` in the user's crate.
        let attrs = &method.attrs;
        let fallback_doc = if has_doc(attrs) {
            quote! {}
        } else {
            let text = format!("Emits the `{sig_name_str}` signal.");
            quote! { #[doc = #text] }
        };

        generated_methods.push(quote! {
            #(#attrs)*
            #fallback_doc
            #vis async fn #sig_name(#(#fn_args),*) {
                self.emitter_core.broadcast(
                    objectspell::Signal::new(self.emitter_core.channel_name.clone(), #sig_name_str)
                        #(#with_params)*
                );
            }
        });
    }

    let block_attrs = &block.attrs;

    let gen = quote! {
        #(#block_attrs)*
        impl #name {
            #(#generated_methods)*
        }

        objectspell::inventory::submit! {
            objectspell::EmitterRegistration {
                target_type: || std::any::TypeId::of::<#name>(),
                routes: &[#(#route_names),*],
            }
        }
    };

    TokenStream::from(gen)
}

// ============================================================================
// state
// ============================================================================

/// The `#[objectspell::state]` macro parses a `pub struct StructName { ... }` definition.
/// It injects `state_core` and `emitter_core` fields, and generates `init` and `into_state`
/// as well as the implementation for `AnyState`.
#[proc_macro_attribute]
pub fn state(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut struct_def = parse_macro_input!(item as ItemStruct);
    let name = &struct_def.ident;
    let channel_name = name.to_string(); // Default to struct name

    let mut init_params = Vec::new();
    let mut init_assigns = Vec::new();

    if let syn::Fields::Named(ref mut fields) = struct_def.fields {
        for f in &fields.named {
            let fname = &f.ident;
            let ftype = &f.ty;
            init_params.push(quote! { #fname: #ftype });
            init_assigns.push(quote! { #fname });
        }

        use syn::parse::Parser;
        // `pub` so generated code can reach them, `doc(hidden)` because they are plumbing:
        // they would otherwise appear on the user's own documentation, and trip `missing_docs`
        // in a crate that denies it.
        fields.named.push(
            syn::Field::parse_named
                .parse2(quote! { #[doc(hidden)] pub state_core: objectspell::StateCore })
                .unwrap(),
        );
        fields.named.push(
            syn::Field::parse_named
                .parse2(quote! { #[doc(hidden)] pub emitter_core: objectspell::EmitterCore })
                .unwrap(),
        );
    } else if let syn::Fields::Unit = struct_def.fields {
        let new_fields: syn::FieldsNamed = syn::parse2(quote! {
            {
                #[doc(hidden)]
                pub state_core: objectspell::StateCore,
                #[doc(hidden)]
                pub emitter_core: objectspell::EmitterCore,
            }
        })
        .unwrap();
        struct_def.fields = syn::Fields::Named(new_fields);
    } else {
        // A `syn::Error` carries a span, so the user gets an ordinary compile error pointing at
        // their struct rather than a macro panic with no source location.
        return syn::Error::new_spanned(
            &struct_def,
            "#[objectspell::state] needs a struct with named fields, or no fields at all. \
             A tuple struct has nowhere to put the queue and emitter it injects.",
        )
        .to_compile_error()
        .into();
    }

    let wrapper_name = format_ident!("{}AnyStateWrapper", name);

    let expanded = quote! {
        #struct_def

        impl #name {
            /// Initializes the component with its fields + auto-injected core fields.
            pub fn init(#(#init_params),*) -> Self {
                Self {
                    #(#init_assigns,)*
                    state_core: objectspell::StateCore::new(),
                    emitter_core: objectspell::EmitterCore::new(#channel_name),
                }
            }

            /// Turn this component into one the Connector can connect.
            ///
            /// Attaches every receiver written for it, then the built-in Connector receiver
            /// that lets `disconnect()` stop it. Normally called for you by `connect()`.
            pub fn into_state(self) -> Box<dyn objectspell::AnyState> {
                let arc_self = std::sync::Arc::new(objectspell::tokio::sync::Mutex::new(self));
                let arc_any = std::sync::Arc::clone(&arc_self) as std::sync::Arc<dyn std::any::Any + Send + Sync>;
                let target_id = std::any::TypeId::of::<#name>();

                if let Ok(mut lock) = arc_self.try_lock() {
                    for reg in objectspell::inventory::iter::<objectspell::DispatcherRegistration> {
                        if (reg.target_type)() == target_id {
                            (reg.register)(&mut lock.state_core, std::sync::Arc::clone(&arc_any));
                        }
                    }
                    // After the hand-written receivers, so that on a shared route theirs runs first.
                    lock.state_core.install_connector_receiver();
                } else {
                    panic!("newly created state cannot already be locked");
                }
                Box::new(#wrapper_name { inner: arc_self })
            }
        }

        impl objectspell::connector::Connectable for #name {
            fn into_states(self, vec: &mut Vec<Box<dyn objectspell::AnyState>>) {
                vec.push(self.into_state());
            }
        }

        impl Default for #name {
            fn default() -> Self {
                Self {
                    #(#init_assigns: Default::default(),)*
                    state_core: objectspell::StateCore::new(),
                    emitter_core: objectspell::EmitterCore::new(#channel_name),
                }
            }
        }

        #[doc(hidden)]
        pub struct #wrapper_name {
            pub inner: std::sync::Arc<objectspell::tokio::sync::Mutex<#name>>
        }

        #[objectspell::async_trait]
        impl objectspell::AnyState for #wrapper_name {
            async fn start_listener(&self) -> Option<objectspell::tokio::task::JoinHandle<()>> {
                self.inner.lock().await.state_core.spawn_listener()
            }
            async fn sender(&self) -> Option<objectspell::tokio::sync::mpsc::UnboundedSender<objectspell::Signal>> {
                let lock = self.inner.lock().await;
                Some(lock.state_core.sender())
            }
            async fn channels(&self) -> Vec<String> {
                let lock = self.inner.lock().await;
                lock.state_core.channels()
            }
            async fn emitter_name(&self) -> String {
                let lock = self.inner.lock().await;
                lock.emitter_core.channel_name.clone()
            }
            async fn emitter_routes(&self) -> Vec<&'static str> {
                objectspell::emitter_routes_of(std::any::TypeId::of::<#name>())
            }
            async fn declared_routes(&self) -> std::collections::HashMap<String, std::collections::BTreeSet<String>> {
                self.inner.lock().await.state_core.declared_routes().clone()
            }
            async fn broadcast(&self, signal: objectspell::Signal) {
                let lock = self.inner.lock().await;
                lock.emitter_core.broadcast(signal);
            }
            async fn connect_receiver(&self, sender: objectspell::tokio::sync::mpsc::UnboundedSender<objectspell::Signal>) {
                let mut lock = self.inner.lock().await;
                lock.emitter_core.connect(sender);
            }
        }
    };

    TokenStream::from(expanded)
}

// ============================================================================
// receiver
// ============================================================================

/// The `#[objectspell::receiver]` macro parses an `impl ChannelName for StructName { ... }` block.
/// It drops the trait bound to make it an inherent impl block on `StructName`, and generates
/// `SignalDispatcher` structs for each method. It then submits an `objectspell::inventory`
/// registration so those dispatchers are attached automatically by `into_state()`.
#[proc_macro_attribute]
pub fn receiver(_args: TokenStream, input: TokenStream) -> TokenStream {
    let mut impl_block = parse_macro_input!(input as ItemImpl);
    let self_ty = &impl_block.self_ty;

    let channel_name = if let Some((_, path, _)) = &impl_block.trait_ {
        path.segments.last().unwrap().ident.to_string()
    } else {
        panic!("Expected impl ChannelName for StructName");
    };

    let self_ty_str_clean = quote!(#self_ty).to_string().replace(" ", "");
    let trait_name = format_ident!("{}ReceiverExtension{}", self_ty_str_clean, channel_name);

    let mut trait_items = Vec::new();
    for item in impl_block.items.iter_mut() {
        if let ImplItem::Fn(method) = item {
            method.vis = Visibility::Inherited;
            let sig = &method.sig;
            trait_items.push(quote! { #sig; });
        }
    }

    let trait_path: syn::Path = syn::parse_str(&trait_name.to_string()).unwrap();
    impl_block.trait_ = Some((
        None,
        trait_path,
        Token![for](proc_macro2::Span::call_site()),
    ));

    let mut dispatchers = Vec::new();
    let mut register_calls = Vec::new();

    for item in impl_block.items.iter() {
        if let ImplItem::Fn(method) = item {
            let sig_name = &method.sig.ident;
            let sig_name_str = sig_name.to_string();
            let struct_name_str = quote!(#self_ty).to_string().replace(" ", "");
            let dispatcher_name = format_ident!("{}Dispatcher{}", struct_name_str, sig_name_str);

            let mut extractors = Vec::new();
            let mut call_args = Vec::new();

            for arg in method.sig.inputs.iter().skip(1) {
                if let syn::FnArg::Typed(pat_type) = arg {
                    if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                        let arg_name = &pat_ident.ident;
                        let arg_name_str = wire_name(arg_name);
                        let arg_ty = &*pat_type.ty;

                        extractors.push(quote! {
                            let #arg_name: #arg_ty = signal.expect_param(#arg_name_str);
                        });
                        call_args.push(quote! { #arg_name });
                    }
                }
            }

            dispatchers.push(quote! {
                struct #dispatcher_name {
                    state: std::sync::Arc<objectspell::tokio::sync::Mutex<#self_ty>>,
                }

                #[objectspell::async_trait]
                impl objectspell::SignalDispatcher for #dispatcher_name {
                    fn channel(&self) -> &'static str {
                        #channel_name
                    }

                    fn route(&self) -> &'static str {
                        #sig_name_str
                    }

                    async fn dispatch(&self, signal: &objectspell::Signal) {
                        // Registered under this channel and route, so the signal is already
                        // the right one. The guard is exclusive, so handlers may take
                        // `&mut self` and mutate the component directly.
                        let mut state = self.state.lock().await;
                        #(#extractors)*
                        <#self_ty as #trait_name>::#sig_name(&mut *state #(, #call_args)*).await;
                    }
                }
            });

            register_calls.push(quote! {
                let typed_arc = arc_any_clone
                    .clone()
                    .downcast::<objectspell::tokio::sync::Mutex<#self_ty>>()
                    .expect("state registered under its own TypeId");
                let dispatcher = Box::new(#dispatcher_name {
                    state: typed_arc,
                });
                core.register(dispatcher);
            });
        }
    }

    let gen = quote! {
        trait #trait_name {
            #(#trait_items)*
        }

        #impl_block

        #(#dispatchers)*

        objectspell::inventory::submit! {
            objectspell::DispatcherRegistration {
                target_type: || std::any::TypeId::of::<#self_ty>(),
                register: |core, arc_any| {
                    let arc_any_clone = std::sync::Arc::clone(&arc_any);
                    #(#register_calls)*
                }
            }
        }
    };

    TokenStream::from(gen)
}
