extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use std::collections::HashMap;
use syn::{parse_macro_input, punctuated::Punctuated, token::Comma, ItemEnum, ItemStruct, Meta};

// Commands
// ========

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
            Build(tracel_xtask_commands::commands::build::BuildCmdArgs)
        },
    );
    variant_map.insert(
        "Bump",
        quote! {
            #[doc = r"Bump the version of all crates to be published."]
            Bump(tracel_xtask_commands::commands::bump::BumpCmdArgs)
        },
    );
    variant_map.insert(
        "Fix",
        quote! {
            #[doc = r"Fix issues found with the 'check' command."]
            Fix(tracel_xtask_commands::commands::fix::FixCmdArgs)
        },
    );
    variant_map.insert(
        "Check",
        quote! {
            #[doc = r"Run checks like formatting, linting etc... This command only reports issues, use the 'fix' command to auto-fix issues."]
            Check(tracel_xtask_commands::commands::check::CheckCmdArgs)
        },
    );
    variant_map.insert(
        "Compile",
        quote! {
            #[doc = r"Compile check the code (does not write binaries to disk)."]
            Compile(tracel_xtask_commands::commands::compile::CompileCmdArgs)
        },
    );
    variant_map.insert(
        "Coverage",
        quote! {
            #[doc = r"Install and run coverage tools."]
            Coverage(tracel_xtask_commands::commands::coverage::CoverageCmdArgs)
        },
    );
    variant_map.insert(
        "Doc",
        quote! {
            #[doc = r"Build documentation."]
            Doc(tracel_xtask_commands::commands::doc::DocCmdArgs)
        },
    );
    variant_map.insert(
        "Dependencies",
        quote! {
            #[doc = r"Run the specified dependencies check locally."]
            Dependencies(tracel_xtask_commands::commands::dependencies::DependenciesCmdArgs)
        },
    );
    variant_map.insert(
        "Publish",
        quote! {
            #[doc = r"Publish a crate to crates.io."]
            Publish(tracel_xtask_commands::commands::publish::PublishCmdArgs)
        },
    );
    variant_map.insert(
        "Test",
        quote! {
            #[doc = r"Runs tests."]
            Test(tracel_xtask_commands::commands::test::TestCmdArgs)
        },
    );
    variant_map.insert(
        "Validate",
        quote! {
            #[doc = r"Validate the code base by running all the relevant checks and tests. Use this command before creating a new pull-request."]
            Validate
        },
    );
    variant_map.insert("Vulnerabilities", quote! {
        #[doc = r"Run the specified vulnerability check locally. These commands must be called with 'cargo +nightly'."]
        Vulnerabilities(tracel_xtask_commands::commands::vulnerabilities::VulnerabilitiesCmdArgs)
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

// Command arguments
// =================

fn generate_command_args_struct(
    args: TokenStream,
    input: TokenStream,
) -> TokenStream {
    let item = parse_macro_input!(input as ItemStruct);
    let args = parse_macro_input!(args with Punctuated::<Meta, Comma>::parse_terminated);

    let struct_name = &item.ident;
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

    if args.is_empty() {
        let output = quote! {
            #[derive(clap::Args, Clone)]
            pub struct #struct_name {
                #(#original_fields,)*
            }
        };
        TokenStream::from(output)
    } else {
        let target_type = if args.len() == 1 {
            args.get(0).unwrap()
        } else if args.len() == 2 {
            args.get(1).unwrap()
        } else {
            return TokenStream::from(quote! {
                compile_error!("Cannot find target type in args_struct.");
            })
        };
        let target_type = quote! { #target_type };

        let output = quote! {
            #[derive(clap::Args, Clone)]
            pub struct #struct_name {
                #[doc = r"The target on which executing the command."]
                #[arg(short, long, value_enum, default_value_t = #target_type::Workspace)]
                pub target: #target_type,
                #[doc = r"Comma-separated list of excluded crates."]
                #[arg(
                    short = 'x',
                    long,
                    value_name = "CRATE,CRATE,...",
                    value_delimiter = ',',
                    required = false
                )]
                pub exclude: Vec<String>,
                #[doc = r"Comma-separated list of crates to include exclusively."]
                #[arg(
                    short = 'n',
                    long,
                    value_name = "CRATE,CRATE,...",
                    value_delimiter = ',',
                    required = false
                )]
                pub only: Vec<String>,
                #(#original_fields,)*
            }
        };
        TokenStream::from(output)
    }
}

fn generate_command_args_tryinto(
    args: TokenStream,
    input: TokenStream,
) -> TokenStream {
    let args = parse_macro_input!(args with Punctuated::<Meta, Comma>::parse_terminated);
    let base_type = args.get(0).unwrap();
    let item = parse_macro_input!(input as ItemStruct);
    let item_ident = &item.ident;
    let has_target = item.fields.iter().any(|f| {
        if let Some(ident) = &f.ident {
            "target" == ident.to_string()
        } else {
            false
        }
    });

    // expand
    let target = if has_target {
        quote! {
            target: self.target.try_into()?
        }
    } else {
        quote! {}
    };
    let fields: Vec<_> = item.fields.iter().filter_map(|f| {
        f.ident.as_ref().map(|ident| {
            let ident_str = ident.to_string();
            if ident_str != "target" {
                quote! { #ident: self.#ident }
            } else {
                quote! {}
            }
        })
    }).collect();

    let tryinto = quote! {
        impl std::convert::TryInto<#base_type> for #item_ident {
            type Error = anyhow::Error;
            fn try_into(self) -> Result<#base_type, Self::Error> {
                Ok(#base_type {
                    #target
                    #(#fields,)*
                })
            }
        }
    };
    TokenStream::from(tryinto)
}

#[proc_macro_attribute]
pub fn declare_command_args(args: TokenStream, input: TokenStream) -> TokenStream {
    let args_clone = args.clone();
    let parsed_args = parse_macro_input!(args_clone with Punctuated::<Meta, Comma>::parse_terminated);
    if parsed_args.len() < 2 {
        generate_command_args_struct(args, input)
    } else {
        TokenStream::from(quote! {
            compile_error!("declare_commands_args macro takes at most 1 argument with is the target type.");
        })
    }
}

#[proc_macro_attribute]
pub fn extend_command_args(args: TokenStream, input: TokenStream) -> TokenStream {
    let args_clone = args.clone();
    let parsed_args = parse_macro_input!(args_clone with Punctuated::<Meta, Comma>::parse_terminated);
    if parsed_args.len() != 2 {
        return TokenStream::from(quote! {
            compile_error!("extend_command_args takes two arguments.\n"
                           "First argument is the base commands arguments struct name.\n"
                           "Second argument is the type of target enum.");
        })
    }
    let mut output = generate_command_args_struct(args.clone(), input);
    let tryinto = generate_command_args_tryinto(args, output.clone());
    output.extend(TokenStream::from(tryinto));
    output
}

// Targets
// =======

fn generate_target_enum(input: TokenStream) -> TokenStream {
    let item = parse_macro_input!(input as ItemEnum);
    let enum_name = &item.ident;
    let original_variants = &item.variants;

    let output = quote! {
        #[derive(strum::EnumString, strum::EnumIter, Default, strum::Display, Clone, PartialEq, clap::ValueEnum)]
        #[strum(serialize_all = "lowercase")]
        pub enum #enum_name {
            #[doc = r"Targets all crates and examples using cargo --package."]
            AllPackages,
            #[doc = r"Targets all binary and library crates."]
            Crates,
            #[doc = r"Targets all example crates."]
            Examples,
            #[default]
            #[doc = r"Targets the whole workspace using cargo --workspace."]
            Workspace,
            #original_variants
        }
    };
    TokenStream::from(output)
}

fn generate_target_tryinto(_args: TokenStream, input: TokenStream) -> TokenStream {
    let item = parse_macro_input!(input as ItemEnum);
    let item_ident = &item.ident;
    let tryinto = quote! {
        impl std::convert::TryInto<tracel_xtask_commands::commands::Target> for #item_ident {
            type Error = anyhow::Error;
            fn try_into(self) -> Result<tracel_xtask_commands::commands::Target, Self::Error> {
                match self {
                    #item_ident::AllPackages => Ok(tracel_xtask_commands::commands::Target::AllPackages),
                    #item_ident::Crates => Ok(tracel_xtask_commands::commands::Target::Crates),
                    #item_ident::Examples => Ok(tracel_xtask_commands::commands::Target::Examples),
                    #item_ident::Workspace => Ok(tracel_xtask_commands::commands::Target::Workspace),
                    _ => Err(anyhow::anyhow!("{} target is not supported.", self))
                }
            }
        }
    };
    TokenStream::from(tryinto)
}

#[proc_macro_attribute]
pub fn declare_targets(_args: TokenStream, input: TokenStream) -> TokenStream {
    generate_target_enum(input)
}

#[proc_macro_attribute]
pub fn extend_targets(args: TokenStream, input: TokenStream) -> TokenStream {
    let mut output = generate_target_enum(input);
    output.extend(generate_target_tryinto(args, output.clone()));
    output
}

