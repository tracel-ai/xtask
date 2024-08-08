extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use std::collections::HashMap;
use syn::{
    parse_macro_input, punctuated::Punctuated, token::Comma, ItemEnum, ItemStruct, Meta, Variant,
};

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

fn generate_command_args_struct(args: TokenStream, input: TokenStream) -> TokenStream {
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
            });
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

fn generate_command_args_tryinto(args: TokenStream, input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args with Punctuated::<Meta, Comma>::parse_terminated);
    let base_type = args.get(0).unwrap();
    let item = parse_macro_input!(input as ItemStruct);
    let item_ident = &item.ident;
    let has_target = item.fields.iter().any(|f| {
        if let Some(ident) = &f.ident {
            *ident == "target"
        } else {
            false
        }
    });
    let has_subcommand = item.fields.iter().any(|f| {
        if let Some(ident) = &f.ident {
            *ident == "command"
        } else {
            false
        }
    });

    // expand
    let target = if has_target {
        quote! {
            target: self.target.try_into()?,
        }
    } else {
        quote! {}
    };
    let subcommand = if has_subcommand {
        quote! {
            command: self.command.try_into()?,
        }
    } else {
        quote! {}
    };
    let fields: Vec<_> = item
        .fields
        .iter()
        .filter_map(|f| {
            f.ident.as_ref().map(|ident| {
                let ident_str = ident.to_string();
                if ident_str != "target" && (ident_str == "exclude" || ident_str == "only") {
                    quote! { #ident: self.#ident, }
                } else {
                    quote! {}
                }
            })
        })
        .collect();

    let tryinto = quote! {
        impl std::convert::TryInto<#base_type> for #item_ident {
            type Error = anyhow::Error;
            fn try_into(self) -> Result<#base_type, Self::Error> {
                Ok(#base_type {
                    #target
                    #subcommand
                    #(#fields)*
                })
            }
        }
    };
    TokenStream::from(tryinto)
}

#[proc_macro_attribute]
pub fn declare_command_args(args: TokenStream, input: TokenStream) -> TokenStream {
    let args_clone = args.clone();
    let parsed_args =
        parse_macro_input!(args_clone with Punctuated::<Meta, Comma>::parse_terminated);
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
    let parsed_args =
        parse_macro_input!(args_clone with Punctuated::<Meta, Comma>::parse_terminated);
    if parsed_args.len() != 2 {
        return TokenStream::from(quote! {
            compile_error!("extend_command_args takes two arguments.\n"
                           "First argument is the base commands arguments struct name.\n"
                           "Second argument is the type of target enum.");
        });
    }
    let mut output = generate_command_args_struct(args.clone(), input);
    let tryinto = generate_command_args_tryinto(args, output.clone());
    output.extend(tryinto);
    output
}

// Subcommands
// ===========

fn get_variant_map() -> HashMap<&'static str, proc_macro2::TokenStream> {
    HashMap::from([
        (
            "Bump",
            quote! {
                #[doc = r"Run unit tests."]
                Major,
                #[doc = r"Run integration tests."]
                Minor,
                #[doc = r"Run documentation tests."]
                Patch,
            },
        ),
        (
            "Check",
            quote! {
                #[default]
                #[doc = r"Run all the checks."]
                All,
                #[doc = r"Run audit command."]
                Audit,
                #[doc = r"Run format command."]
                Format,
                #[doc = r"Run lint command."]
                Lint,
                #[doc = r"Report typos in source code."]
                Typos,
            },
        ),
        (
            "Coverage",
            quote! {
                #[doc = r"Install grcov and its dependencies."]
                Install,
                #[doc = r"Generate lcov.info file."]
                Generate(GenerateCmdArgs),
            },
        ),
        (
            "Dependencies",
            quote! {
                #[doc = r"Run all dependency checks."]
                #[default]
                All,
                #[doc = r"Run cargo-deny Lint dependency graph to ensure all dependencies meet requirements `<https://crates.io/crates/cargo-deny>`"]
                Deny,
                #[doc = r"Run cargo-udeps to find unused dependencies `<https://crates.io/crates/cargo-udeps>`"]
                Unused,
            },
        ),
        (
            "Doc",
            quote! {
                #[default]
                #[doc = r"Build documentation."]
                Build,
                #[doc = r"Run documentation tests."]
                Tests,
            },
        ),
        (
            "Fix",
            quote! {
                #[default]
                #[doc = r"Run all the checks."]
                All,
                #[doc = r"Run audit command."]
                Audit,
                #[doc = r"Run format command and fix formatting."]
                Format,
                #[doc = r"Run lint command and fix issues."]
                Lint,
                #[doc = r"Find typos in source code and fix them."]
                Typos,
            },
        ),
        (
            "Test",
            quote! {
                #[default]
                #[doc = r"Run all the checks."]
                All,
                #[doc = r"Run unit tests."]
                Unit,
                #[doc = r"Run integration tests."]
                Integration,
            },
        ),
        (
            "Vulnerabilities",
            quote! {
                #[default]
                #[doc = r"Run all most useful vulnerability checks."]
                All,
                #[doc = r"Run Address sanitizer (memory error detector)"]
                AddressSanitizer,
                #[doc = r"Run LLVM Control Flow Integrity (CFI) (provides forward-edge control flow protection)"]
                ControlFlowIntegrity,
                #[doc = r"Run newer variant of Address sanitizer (memory error detector similar to AddressSanitizer, but based on partial hardware assistance)"]
                HWAddressSanitizer,
                #[doc = r"Run Kernel LLVM Control Flow Integrity (KCFI) (provides forward-edge control flow protection for operating systems kernels)"]
                KernelControlFlowIntegrity,
                #[doc = r"Run Leak sanitizer (run-time memory leak detector)"]
                LeakSanitizer,
                #[doc = r"Run memory sanitizer (detector of uninitialized reads)"]
                MemorySanitizer,
                #[doc = r"Run another address sanitizer (like AddressSanitizer and HardwareAddressSanitizer but with lower overhead suitable for use as hardening for production binaries)"]
                MemTagSanitizer,
                #[doc = r"Run nightly-only checks through cargo-careful `<https://crates.io/crates/cargo-careful>`"]
                NightlyChecks,
                #[doc = r"Run SafeStack check (provides backward-edge control flow protection by separating stack into safe and unsafe regions"]
                SafeStack,
                #[doc = r"Run ShadowCall check (provides backward-edge control flow protection - aarch64 only)"]
                ShadowCallStack,
                #[doc = r"Run Thread sanitizer (data race detector)"]
                ThreadSanitizer,
            },
        ),
    ])
}

fn generate_subcommand_enum(args: TokenStream, input: TokenStream) -> TokenStream {
    let item = parse_macro_input!(input as ItemEnum);
    let args = parse_macro_input!(args with Punctuated::<Meta, Comma>::parse_terminated);

    // First argument is the name of the command
    // Boundaries check is performed by the caller.
    let command = args.get(0).unwrap();
    let base_command_ident = command.path().get_ident().unwrap();
    let base_command_string = base_command_ident.to_string();
    let enum_name = &item.ident;
    let original_variants = &item.variants;

    let variant_map = get_variant_map();
    let output = if let Some(variants) = variant_map.get(base_command_string.as_str()) {
        // parse the variant and look for a default attribute so that we add the default derive if required
        let variants_tokens = TokenStream::from(variants.clone());
        let parsed_variants =
            parse_macro_input!(variants_tokens with Punctuated::<Variant, Comma>::parse_terminated);
        let default = if parsed_variants
            .iter()
            .any(|v| v.attrs.iter().any(|a| a.path().is_ident("default")))
        {
            quote! { Default }
        } else {
            quote! {}
        };
        quote! {
            #[derive(strum::EnumString, strum::EnumIter, strum::Display, Clone, PartialEq, clap::Subcommand, #default)]
            #[strum(serialize_all = "lowercase")]
            pub enum #enum_name {
                #variants
                #original_variants
            }
        }
    } else {
        let err_msg = format!(
            "Unknown command: {}\nPossible commands are:\n  {}",
            base_command_string,
            variant_map
                .keys()
                .cloned()
                .collect::<Vec<&str>>()
                .join("\n  "),
        );
        quote! { compile_error!(#err_msg); }
    };

    TokenStream::from(output)
}

fn generate_subcomand_tryinto(args: TokenStream, input: TokenStream) -> TokenStream {
    let item = parse_macro_input!(input as ItemEnum);
    let args = parse_macro_input!(args with Punctuated::<Meta, Comma>::parse_terminated);
    // First argument is the name of the command
    // Boundaries check is performed by the caller.
    let command = args.get(0).unwrap();
    let base_command_type = args.get(1).unwrap();
    let base_command_ident = command.path().get_ident().unwrap();
    let base_command_string = base_command_ident.to_string();
    let extend_command_type = &item.ident;

    let variant_map = get_variant_map();
    let tryinto = if let Some(variants) = variant_map.get(base_command_string.as_str()) {
        // parse the variant and look for a default attribute so that we add the default derive if required
        let variants_tokens = TokenStream::from(variants.clone());
        let parsed_variants =
            parse_macro_input!(variants_tokens with Punctuated::<Variant, Comma>::parse_terminated);
        let arms = parsed_variants.iter().map(|v| {
            let variant_ident = &v.ident;
            quote! {
                 #extend_command_type::#variant_ident => Ok(#base_command_type::#variant_ident),
            }
        });
        quote! {
            impl std::convert::TryInto<#base_command_type> for #extend_command_type {
                type Error = anyhow::Error;
                fn try_into(self) -> Result<#base_command_type, Self::Error> {
                    match self {
                        #(#arms)*
                        _ => Err(anyhow::anyhow!("{} target is not supported.", self))
                    }
                }
            }
        }
    } else {
        let err_msg = format!(
            "Unknown command: {}\nPossible commands are:\n  {}",
            base_command_string,
            variant_map
                .keys()
                .cloned()
                .collect::<Vec<&str>>()
                .join("\n  "),
        );
        quote! { compile_error!(#err_msg); }
    };
    TokenStream::from(tryinto)
}

#[proc_macro_attribute]
pub fn declare_subcommand(args: TokenStream, input: TokenStream) -> TokenStream {
    let args_clone = args.clone();
    let parsed_args =
        parse_macro_input!(args_clone with Punctuated::<Meta, Comma>::parse_terminated);
    if parsed_args.len() != 1 {
        return TokenStream::from(quote! {
            compile_error!("declare_subcommand takes one argument which the name of the base command name.\n");
        });
    }
    generate_subcommand_enum(args, input)
}

#[proc_macro_attribute]
pub fn extend_subcommand(args: TokenStream, input: TokenStream) -> TokenStream {
    let args_clone = args.clone();
    let parsed_args =
        parse_macro_input!(args_clone with Punctuated::<Meta, Comma>::parse_terminated);
    if parsed_args.len() != 2 {
        return TokenStream::from(quote! {
            compile_error!("extend_subcommand takes two arguments.\n"
                           "The first one is the base command name.\n"
                           "The second one is the base command type.\n");
        });
    }
    let mut output = generate_subcommand_enum(args.clone(), input);
    output.extend(generate_subcomand_tryinto(args, output.clone()));
    output
}
