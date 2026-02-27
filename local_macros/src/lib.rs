use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    parenthesized,
    Ident, Token, Type,
};

// 一行分の定義: func_name, (args...), ReturnType
struct ApiDefinition {
    name: Ident,
    args: Punctuated<Type, Token![,]>,
    ret_type: Type,
}

// 複数の定義をまとめる
struct ApiDefinitions {
    defs: Punctuated<ApiDefinition, Token![,]>,
}

impl Parse for ApiDefinitions {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut defs = Punctuated::new();
        while !input.is_empty() {
            let name: Ident = input.parse()?;
            input.parse::<Token![,]>()?;

            // カッコ内の型リストをパース
            let content;
            parenthesized!(content in input);
            let args = content.parse_terminated(Type::parse, Token![,])?;

            input.parse::<Token![,]>()?;
            let ret_type: Type = input.parse()?;

            defs.push(ApiDefinition { name, args, ret_type });

            // 次の定義がある場合はカンマを消費
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }
        Ok(ApiDefinitions { defs })
    }
}

#[proc_macro]
pub fn define_api(input: TokenStream) -> TokenStream {
    let ApiDefinitions { defs } = parse_macro_input!(input as ApiDefinitions);

    let mut struct_fields = Vec::new();
    let mut static_inits = Vec::new();
    let mut extern_decls = Vec::new();

    for def in defs {
        let name = &def.name;
        let ret_type = &def.ret_type;
        let arg_types: Vec<_> = def.args.iter().collect();

        // 1. 構造体のフィールド型: extern "C" fn(u8, u32) -> String
        struct_fields.push(quote! {
            pub #name: extern "C" fn(#(#arg_types),*) -> #ret_type
        });

        // 2. static変数の初期化: func_1: func_1
        static_inits.push(quote! {
            #name: #name
        });

        // 3. extern "C" ブロック用の引数名生成: (arg0: u8, arg1: u32)
        let arg_idents: Vec<_> = (0..arg_types.len())
            .map(|i| format_ident!("arg{}", i))
            .collect();

        extern_decls.push(quote! {
            pub fn #name(#(#arg_idents: #arg_types),*) -> #ret_type;
        });
    }

    quote! {
        #[repr(C)]
        pub struct ApiTable {
            #(#struct_fields),*
        }

        #[unsafe(no_mangle)]
        pub static API_TABLE: ApiTable = ApiTable {
            #(#static_inits),*
        };
    }.into()
}

#[proc_macro]
pub fn call_api(input: TokenStream) -> TokenStream {
    let ApiDefinitions { defs } = parse_macro_input!(input as ApiDefinitions);

    let mut struct_fields = Vec::new();

    for def in defs {
        let name = &def.name;
        let ret_type = &def.ret_type;
        let arg_types: Vec<_> = def.args.iter().collect();

        struct_fields.push(quote! {
            pub #name: extern "C" fn(#(#arg_types),*) -> #ret_type
        });
    }

    quote! {
        #[repr(C)]
        pub struct ApiTable {
            #(#struct_fields),*
        }

        impl ApiTable {
            pub unsafe fn from_ptr(ptr: *const u8) -> &'static Self {
                &*(ptr as *const Self)
            }
        }
    }.into()
}