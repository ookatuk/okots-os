use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    braced,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    token, Ident, Token,
};

enum FlagTree {
    Leaf(Ident),
    Node(FlagGroup),
}

struct FlagGroup {
    name: Ident,
    _brace_token: token::Brace,
    body: Punctuated<FlagTree, Token![,]>,
}

impl Parse for FlagTree {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        if input.peek(token::Brace) {
            let content;
            Ok(FlagTree::Node(FlagGroup {
                name,
                _brace_token: braced!(content in input),
                body: content.parse_terminated(FlagTree::parse, Token![,])?,
            }))
        } else {
            Ok(FlagTree::Leaf(name))
        }
    }
}

struct FlagRoot {
    flags: Punctuated<FlagTree, Token![,]>,
}

impl Parse for FlagRoot {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(FlagRoot {
            flags: input.parse_terminated(FlagTree::parse, Token![,])?,
        })
    }
}

#[proc_macro]
pub fn define_cpu_flags(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as FlagRoot);

    let mut current_id = 0u32;
    let mut flat_list: Vec<(u32, Ident)> = Vec::new();

    // 再帰的に mod 構造を生成
    fn expand_tree(
        tree: &FlagTree,
        id_counter: &mut u32,
        flat_list: &mut Vec<(u32, Ident)>,
        prefix: String,
    ) -> proc_macro2::TokenStream {
        match tree {
            FlagTree::Leaf(name) => {
                let id = *id_counter;
                *id_counter += 1;
                let variant_name = format_ident!("{}{}", prefix, name);
                flat_list.push((id, variant_name.clone()));

                // 絶対パス的な解決を避けるため、super の連鎖ではなく
                // 常に一つ上の flags モジュールから見える名前空間を利用する
                quote! {
                    pub const #name: super::CpuFlag =
                        super::CpuFlag(super::InternalFlagKind::#variant_name);
                }
            }
            FlagTree::Node(group) => {
                let name = &group.name;
                let new_prefix = format!("{}{}_", prefix, name);
                let children: Vec<_> = group.body.iter()
                    .map(|child| expand_tree(child, id_counter, flat_list, new_prefix.clone()))
                    .collect();

                quote! {
                    pub mod #name {
                        use super::*;
                        #(#children)*
                    }
                }
            }
        }
    }

    let modules: Vec<_> = input.flags.iter()
        .map(|f| expand_tree(f, &mut current_id, &mut flat_list, String::new()))
        .collect();

    let count = current_id as usize;
    let num_u64 = (count + 63) / 64;

    let internal_variants = flat_list.iter().map(|(id, variant_name)| {
        quote! { #[allow(non_camel_case_types)] #variant_name = #id }
    });

    let match_arms = flat_list.iter().map(|(_id, variant_name)| {
        quote! {
            InternalFlagKind::#variant_name => raw_detect_flag_impl(InternalFlagKind::#variant_name)
        }
    });

    let expanded = quote! {
        #[repr(u32)]
        #[derive(Debug, Copy, Clone, PartialEq, Eq)]
        pub enum InternalFlagKind {
            // Default 用に None または 最初の要素を確保
            #(#internal_variants),*
        }

        // ThreadLocal 等で必要なため Default を実装
        #[derive(Debug, Copy, Clone, PartialEq, Eq)]
        pub struct CpuFlag(pub(crate) InternalFlagKind);

        impl Default for CpuFlag {
            fn default() -> Self {
                // 最初のフラグをデフォルトとする（または InternalFlagKind に None を作るべきだが、
                // 現状の構成に合わせて最初のバリアントを安全に指定）
                unsafe { core::mem::transmute(0u32) }
            }
        }

        impl CpuFlag {
            pub const fn kind(&self) -> InternalFlagKind {
                self.0
            }
        }

        // フラグ定数の階層
        pub mod flags {
            use super::{CpuFlag, InternalFlagKind};
            #(#modules)*
        }

        #[derive(Default)]
        pub struct CpuFlagCache {
            status: [core::sync::atomic::AtomicU64; #num_u64],
            values: [core::sync::atomic::AtomicU64; #num_u64],
        }

        impl CpuFlagCache {
            pub const fn new() -> Self {
                const INIT: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
                Self {
                    status: [INIT; #num_u64],
                    values: [INIT; #num_u64],
                }
            }

            pub fn has(&self, flag: CpuFlag) -> bool {
                let id = flag.0 as u32 as usize;
                let index = id / 64;
                let bit = id % 64;
                let mask = 1u64 << bit;

                let loaded = self.status[index].load(core::sync::atomic::Ordering::Acquire);
                if (loaded & mask) != 0 {
                    let vals = self.values[index].load(core::sync::atomic::Ordering::Relaxed);
                    return (vals & mask) != 0;
                }

                let result = match flag.0 {
                    #(#match_arms,)*
                };

                if result {
                    self.values[index].fetch_or(mask, core::sync::atomic::Ordering::Relaxed);
                } else {
                    self.values[index].fetch_and(!mask, core::sync::atomic::Ordering::Relaxed);
                }
                self.status[index].fetch_or(mask, core::sync::atomic::Ordering::Release);
                result
            }
        }
    };

    TokenStream::from(expanded)
}