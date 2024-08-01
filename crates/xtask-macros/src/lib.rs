extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use std::collections::HashMap;
use syn::{parse_macro_input, punctuated::Punctuated, token::Comma, ItemEnum, ItemStruct, Meta};

#[proc_macro_attribute]
pub fn commands(args: TokenStream, input: TokenStream) -> TokenStream {
    // Parse the input tokens into a syntax tree
    let item = parse_macro_input!(input as ItemEnum);
    let args = parse_macro_input!(args with Punctuated::<Meta, Comma>::parse_terminated);

    // Supported commands and their quoted expansions
    let mut variant_map: HashMap<&str, proc_macro2::TokenStream> = HashMap::new();
    variant_map.insert(
        "Build",
        quote! {
            #[doc = r"Build the code."]
            Build(xtask_common::commands::build::BuildCmdArgs)
        },
    );
    variant_map.insert(
        "Bump",
        quote! {
            #[doc = r"Bump the version of all crates to be published."]
            Bump(xtask_common::commands::bump::BumpCmdArgs)
        },
    );
    variant_map.insert(
        "Fix",
        quote! {
            #[doc = r"Run checks and try to fix the detected issues."]
            Fix(xtask_common::commands::fix::FixCmdArgs)
        },
    );
    variant_map.insert(
        "Checks",
        quote! {
            #[doc = r"Runs all the checks that should pass before creating a pull-request."]
            Checks(xtask_common::commands::checks::ChecksCmdArgs)
        },
    );
    variant_map.insert(
        "CI",
        quote! {
            #[doc = r"Runs checks for Continuous Integration."]
            CI(xtask_common::commands::ci::CICmdArgs)
        },
    );
    variant_map.insert(
        "Compile",
        quote! {
            #[doc = r"Compile check the code (does not write binaries to disk)."]
            Compile(xtask_common::commands::compile::CompileCmdArgs)
        },
    );
    variant_map.insert(
        "Coverage",
        quote! {
            #[doc = r"Install and run coverage tools."]
            Coverage(xtask_common::commands::coverage::CoverageCmdArgs)
        },
    );
    variant_map.insert(
        "Doc",
        quote! {
            #[doc = r"Build documentation."]
            Doc(xtask_common::commands::doc::DocCmdArgs)
        },
    );
    variant_map.insert(
        "Dependencies",
        quote! {
            #[doc = r"Run the specified dependencies check locally."]
            Dependencies(xtask_common::commands::dependencies::DependenciesCmdArgs)
        },
    );
    variant_map.insert(
        "Publish",
        quote! {
            #[doc = r"Publish a crate to crates.io."]
            Publish(xtask_common::commands::publish::PublishCmdArgs)
        },
    );
    variant_map.insert(
        "Test",
        quote! {
            #[doc = r"Runs tests."]
            Test(xtask_common::commands::test::TestCmdArgs)
        },
    );
    variant_map.insert("Vulnerabilities", quote! {
        #[doc = r"Run the specified vulnerability check locally. These commands must be called with 'cargo +nightly'."]
        Vulnerabilities(xtask_common::commands::vulnerabilities::VulnerabilitiesCmdArgs)
    });

    // Generate the corresponding enum variant
    let mut variants = vec![];
    for arg in args {
        if let Meta::Path(path) = arg {
            if let Some(ident) = path.get_ident() {
                let ident_string = ident.to_string();
                if let Some(variant) = variant_map.get(ident_string.as_str()) {
                    variants.push(variant.clone());
                } else {
                    let err_msg = format!(
                        "Unknown command: {}\nPossible commands are:\n  {}",
                        ident_string,
                        variant_map
                            .keys()
                            .cloned()
                            .collect::<Vec<&str>>()
                            .join("\n  "),
                    );
                    return TokenStream::from(quote! {
                        compile_error!(#err_msg);
                    });
                }
            }
        }
    }

    // Generate the xtask commands enum
    let enum_name = &item.ident;
    let other_variants = &item.variants;
    let expanded = quote! {
        #[derive(clap::Subcommand)]
        pub enum #enum_name {
            #(#variants,)*
            #other_variants
        }
    };
    TokenStream::from(expanded)
}

#[proc_macro_attribute]
pub fn arguments(args: TokenStream, input: TokenStream) -> TokenStream {
    let item = parse_macro_input!(input as ItemStruct);
    let args = parse_macro_input!(args with Punctuated::<Meta, Comma>::parse_terminated);

    let mut field_map: HashMap<&str, proc_macro2::TokenStream> = HashMap::new();
    field_map.insert(
        "target",
        quote! {
            #[doc = r"The target on which executing the command."]
            #[arg(short, long, value_enum, default_value_t = Target::Workspace)]
            pub target: Target
        },
    );
    field_map.insert(
        "exclude",
        quote! {
            #[doc = r"Comma-separated list of excluded crates."]
            #[arg(
                short = 'x',
                long,
                value_name = "CRATE,CRATE,...",
                value_delimiter = ',',
                required = false
            )]
            pub exclude: Vec<String>
        },
    );
    field_map.insert(
        "only",
        quote! {
            #[doc = r"Comma-separated list of crates to include exclusively."]
            #[arg(
                short = 'n',
                long,
                value_name = "CRATE,CRATE,...",
                value_delimiter = ',',
                required = false
            )]
            pub only: Vec<String>
        },
    );

    let mut fields = vec![];
    for arg in args {
        if let Meta::Path(path) = arg {
            if let Some(ident) = path.get_ident() {
                let ident_string = ident.to_string();
                if let Some(field) = field_map.get(ident_string.as_str()) {
                    fields.push(field.clone());
                } else {
                    let err_msg = format!(
                        "Unknown argument: {}\nPossible arguments are:\n  {}",
                        ident_string,
                        field_map
                            .keys()
                            .cloned()
                            .collect::<Vec<&str>>()
                            .join("\n  "),
                    );
                    return TokenStream::from(quote! {
                        compile_error!(#err_msg);
                    });
                }
            }
        }
    }

    let struct_name = &item.ident;
    // we quote each componnets of each field manually to avoid
    // having the wrapping curly braces of the struct
    let original_fields = item.fields.iter().map(|f| {
        let attrs = &f.attrs;
        let vis = &f.vis;
        let ident = &f.ident;
        let ty = &f.ty;
        quote! {
            #(#attrs)*
            #vis #ident: #ty
        }
    });

    let expanded = quote! {
        #[derive(clap::Args, Clone)]
        pub struct #struct_name {
            #(#fields,)*
            #(#original_fields,)*
        }
    };
    TokenStream::from(expanded)
}
