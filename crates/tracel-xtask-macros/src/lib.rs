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
        impl std::convert::TryInto<tracel_xtask::commands::Target> for #item_ident {
            type Error = anyhow::Error;
            fn try_into(self) -> Result<tracel_xtask::commands::Target, Self::Error> {
                match self {
                    #item_ident::AllPackages => Ok(tracel_xtask::commands::Target::AllPackages),
                    #item_ident::Crates => Ok(tracel_xtask::commands::Target::Crates),
                    #item_ident::Examples => Ok(tracel_xtask::commands::Target::Examples),
                    #item_ident::Workspace => Ok(tracel_xtask::commands::Target::Workspace),
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

fn generate_dispatch_function(
    enum_ident: &syn::Ident,
    args: &Punctuated<Meta, Comma>,
) -> TokenStream {
    let arms: Vec<proc_macro2::TokenStream> = args.iter().map(|meta| {
        let cmd_ident = meta.path().get_ident().unwrap();
        let cmd_ident_string = cmd_ident.to_string();
        let module_ident = syn::Ident::new(cmd_ident_string.to_lowercase().as_str(), cmd_ident.span());
        match cmd_ident_string.as_str() {
            "Fix" => quote! {
                #enum_ident::#cmd_ident(cmd_args) => base_commands::#module_ident::handle_command(cmd_args, args.environment, args.context, None),
            },
            _ => quote! {
                #enum_ident::#cmd_ident(cmd_args) => base_commands::#module_ident::handle_command(cmd_args, args.environment, args.context),
            }
        }
    }).collect();
    let func = quote! {
        fn dispatch_base_commands(args: XtaskArgs<Command>) -> anyhow::Result<()> {
            match args.command {
                #(#arms)*
                _ => Err(anyhow::anyhow!("Unknown command")),
            }
        }
    };
    TokenStream::from(func)
}

#[proc_macro_attribute]
pub fn base_commands(args: TokenStream, input: TokenStream) -> TokenStream {
    // Parse the input tokens into a syntax tree
    let item = parse_macro_input!(input as ItemEnum);
    let args = parse_macro_input!(args with Punctuated::<Meta, Comma>::parse_terminated);

    // Supported commands and their quoted expansions
    let mut variant_map: HashMap<&str, proc_macro2::TokenStream> = HashMap::new();
    variant_map.insert(
        "Build",
        quote! {
            #[doc = r"Build the code."]
            Build(tracel_xtask::commands::build::BuildCmdArgs)
        },
    );
    variant_map.insert(
        "Bump",
        quote! {
            #[doc = r"Bump the version of all crates to be published."]
            Bump(tracel_xtask::commands::bump::BumpCmdArgs)
        },
    );
    variant_map.insert(
        "Check",
        quote! {
            #[doc = r"Run checks like formatting, linting etc... This command only reports issues, use the 'fix' command to auto-fix issues."]
            Check(tracel_xtask::commands::check::CheckCmdArgs)
        },
    );
    variant_map.insert(
        "Compile",
        quote! {
            #[doc = r"Compile check the code (does not write binaries to disk)."]
            Compile(tracel_xtask::commands::compile::CompileCmdArgs)
        },
    );
    variant_map.insert(
        "Coverage",
        quote! {
            #[doc = r"Install and run coverage tools."]
            Coverage(tracel_xtask::commands::coverage::CoverageCmdArgs)
        },
    );
    variant_map.insert(
        "Dependencies",
        quote! {
            #[doc = r"Run the specified dependencies check locally."]
            Dependencies(tracel_xtask::commands::dependencies::DependenciesCmdArgs)
        },
    );
    variant_map.insert(
        "Doc",
        quote! {
            #[doc = r"Build documentation."]
            Doc(tracel_xtask::commands::doc::DocCmdArgs)
        },
    );
    variant_map.insert(
        "Docker",
        quote! {
            #[doc = r"Manage docker compose stacks."]
            Docker(tracel_xtask::commands::docker::DockerCmdArgs)
        },
    );
    variant_map.insert(
        "Fix",
        quote! {
            #[doc = r"Fix issues found with the 'check' command."]
            Fix(tracel_xtask::commands::fix::FixCmdArgs)
        },
    );
    variant_map.insert(
        "Publish",
        quote! {
            #[doc = r"Publish a crate to crates.io."]
            Publish(tracel_xtask::commands::publish::PublishCmdArgs)
        },
    );
    variant_map.insert(
        "Test",
        quote! {
            #[doc = r"Runs tests."]
            Test(tracel_xtask::commands::test::TestCmdArgs)
        },
    );
    variant_map.insert(
        "Validate",
        quote! {
            #[doc = r"Validate the code base by running all the relevant checks and tests. Use this command before creating a new pull-request."]
            Validate(tracel_xtask::commands::validate::ValidateCmdArgs)
        },
    );
    variant_map.insert("Vulnerabilities", quote! {
        #[doc = r"Run the specified vulnerability check locally. These commands must be called with 'cargo +nightly'."]
        Vulnerabilities(tracel_xtask::commands::vulnerabilities::VulnerabilitiesCmdArgs)
    });

    // Generate the corresponding enum variant
    let mut variants = vec![];
    for arg in &args {
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
    let mut output = TokenStream::from(quote! {
        #[derive(clap::Subcommand)]
        pub enum #enum_name {
            #(#variants,)*
            #other_variants
        }
    });
    output.extend(generate_dispatch_function(enum_name, &args));
    output
}

// Command arguments
// =================

fn get_additional_cmd_args_map() -> HashMap<&'static str, proc_macro2::TokenStream> {
    HashMap::from([
        (
            "BuildCmdArgs",
            quote! {
                #[doc = r"Build artifacs in release mode."]
                #[arg(short, long, required = false)]
                pub release: bool,
            },
        ),
        (
            "CheckCmdArgs",
            quote! {
                #[doc = r"Ignore audit errors."]
                #[arg(long = "ignore-audit", required = false)]
                pub ignore_audit: bool,
            },
        ),
        (
            "DockerCmdArgs",
            quote! {
                #[doc = r"Build images before starting containers."]
                #[arg(short, long, required = false)]
                pub build: bool,
                #[doc = r"Project name."]
                #[arg(short, long, default_value = "xtask")]
                pub project: String,
                #[doc = r"Space separated list of service subset to start. If empty then launch all the services in the stack."]
                #[arg(short, long, num_args(1..), required = false)]
                pub services: Vec<String>,
            },
        ),
        (
            "TestCmdArgs",
            quote! {
                #[doc = r"Execute only the test whose name matches the passed string."]
                #[arg(
                    long = "test",
                    value_name = "TEST",
                    required = false
                )]
                pub test: Option<String>,
                #[doc = r"Maximum number of parallel test crate compilations."]
                #[arg(
                    long = "compilation-jobs",
                    value_name = "NUMBER OF THREADS",
                    required = false
                )]
                pub jobs: Option<u16>,
                #[doc = r"Maximum number of parallel test within a test crate execution."]
                #[arg(
                    long = "test-threads",
                    value_name = "NUMBER OF THREADS",
                    required = false
                )]
                pub threads: Option<u16>,
                #[doc = r"Comma-separated list of features to enable during tests."]
                #[arg(
                    long,
                    value_name = "FEATURE,FEATURE,...",
                    value_delimiter = ',',
                    required = false
                )]
                pub features: Option<Vec<String>>,
                #[doc = r"If set, ignore default features."]
                #[arg(
                    long = "no-default-features",
                    required = false
                )]
                pub no_default_features: bool,
                #[doc = r"Force execution of tests no matter the environment (i.e. authorize to execute tests in prod)."]
                #[arg(
                    short = 'f',
                    long = "force",
                    required = false
                )]
                pub force: bool,
                #[doc = r"If set, test logs are sent to output."]
                #[arg(long = "nocapture", required = false)]
                pub no_capture: bool,
            },
        ),
        (
            "ValidateCmdArgs",
            quote! {
                #[doc = r"Ignore audit errors."]
                #[arg(long = "ignore-audit", required = false)]
                pub ignore_audit: bool,
            },
        ),
    ])
}

// Returns a tuple where 0 is the actual struct and 1 is additional implementations
fn generate_command_args_struct(
    args: TokenStream,
    input: TokenStream,
) -> (TokenStream, TokenStream) {
    let item = match syn::parse::<ItemStruct>(input) {
        Ok(data) => data,
        Err(e) => return (TokenStream::from(e.to_compile_error()), TokenStream::new()),
    };
    let args = match syn::parse::Parser::parse(Punctuated::<Meta, Comma>::parse_terminated, args) {
        Ok(data) => data,
        Err(e) => return (TokenStream::from(e.to_compile_error()), TokenStream::new()),
    };
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
        let struct_output = TokenStream::from(quote! {
            #[derive(clap::Args, Clone)]
            pub struct #struct_name {
                #(#original_fields,)*
            }
        });
        (struct_output, TokenStream::new())
    } else {
        let mut target_type: Option<Meta> = None;
        let mut subcommand_type: Option<Meta> = None;
        if args.len() == 2 {
            // from declare_command_args
            let ty = args.get(0).unwrap();
            if ty.path().get_ident().unwrap().to_string().as_str() != "None" {
                target_type = Some(ty.clone());
            }
            let ty = args.get(1).unwrap();
            if ty.path().get_ident().unwrap().to_string().as_str() != "None" {
                subcommand_type = Some(ty.clone());
            }
        } else if args.len() == 3 {
            // from extend_command_args
            let ty = args.get(1).unwrap();
            if ty.path().get_ident().unwrap().to_string().as_str() != "None" {
                target_type = Some(ty.clone());
            }
            let ty = args.get(2).unwrap();
            if ty.path().get_ident().unwrap().to_string().as_str() != "None" {
                subcommand_type = Some(ty.clone());
            }
        } else {
            return (
                TokenStream::from(quote! {
                    compile_error!("Error expanding macro.");
                }),
                TokenStream::new(),
            );
        };

        let target_fields = if let Some(target) = target_type {
            quote! {
                #[doc = r"The target on which executing the command."]
                #[arg(short, long, value_enum, default_value_t = #target::default())]
                pub target: #target,
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
            }
        } else {
            quote! {}
        };

        let additional_cmd_args_map = get_additional_cmd_args_map();
        let mut base_command_type = struct_name.to_string();
        if args.len() == 3 {
            base_command_type = args.get(0).unwrap().path().get_ident().unwrap().to_string();
        }
        let additional_fields = match additional_cmd_args_map.get(base_command_type.as_str()) {
            Some(fields) => fields.clone(),
            None => quote! {},
        };

        let (subcommand_field, subcommand_impl) = if let Some(subcommand) = subcommand_type.clone()
        {
            (
                quote! {
                    #[command(subcommand)]
                    pub command: Option<#subcommand>,
                },
                quote! {
                    impl #struct_name {
                        pub fn get_command(&self) -> #subcommand {
                            self.command.clone().unwrap_or_default()
                        }
                    }
                },
            )
        } else {
            (quote! {}, quote! {})
        };

        let struct_output = TokenStream::from(quote! {
            #[derive(clap::Args, Clone)]
            pub struct #struct_name {
                #target_fields
                #additional_fields
                #subcommand_field
                #(#original_fields,)*
            }
        });
        let mut additional_output = TokenStream::from(quote! {
            #subcommand_impl
        });
        // generate the subcommand enum only when it is declared
        if args.len() == 2 {
            if let Some(subcommand) = subcommand_type {
                let subcommand_ident = subcommand.path().get_ident().unwrap();
                let subcommand_string = subcommand_ident.to_string();
                let original_variants = Punctuated::<Variant, Comma>::new();
                additional_output.extend(generate_subcommand_enum(
                    subcommand_string,
                    subcommand_ident,
                    &original_variants,
                ));
            }
        }
        (struct_output, additional_output)
    }
}

fn generate_command_args_tryinto(args: TokenStream, input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args with Punctuated::<Meta, Comma>::parse_terminated);
    let base_type = args.get(0).unwrap();
    let base_type_string = base_type.path().get_ident().unwrap().to_string();
    let item = parse_macro_input!(input as ItemStruct);
    let item_ident = &item.ident;
    let has_target = item.fields.iter().any(|f| {
        if let Some(ident) = &f.ident {
            *ident == "target"
        } else {
            false
        }
    });
    // check if the base command has subcommands
    let subcommand_variant_map = get_subcommand_variant_map();
    let base_subcommand_type_string = base_type_string.replace("CmdArgs", "SubCommand");
    let has_subcommand = subcommand_variant_map.contains_key(base_subcommand_type_string.as_str())
        && item.fields.iter().any(|f| {
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
    let (subcommand_let, subcommand_assign) = if has_subcommand {
        (
            quote! {
                let cmd = self.get_command().try_into()?;
            },
            quote! {
                command: Some(cmd),
            },
        )
    } else {
        (quote! {}, quote! {})
    };
    let fields: Vec<_> = item
        .fields
        .iter()
        .filter_map(|f| {
            f.ident.as_ref().map(|ident| {
                let ident_str = ident.to_string();
                // TODO this hardcoded predicate is awful, find a way to make this better
                if ident_str != "target"
                    && (ident_str == "exclude"
                        || ident_str == "features"
                        || ident_str == "force"
                        || ident_str == "ignore_audit"
                        || ident_str == "jobs"
                        || ident_str == "no_default_features"
                        || ident_str == "no_capture"
                        || ident_str == "only"
                        || ident_str == "release"
                        || ident_str == "test"
                        || ident_str == "threads")
                {
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
                #subcommand_let
                Ok(#base_type {
                    #target
                    #subcommand_assign
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
    if parsed_args.len() == 2 {
        let mut output: TokenStream = quote! {}.into();
        let (struct_output, additional_output) = generate_command_args_struct(args, input);
        output.extend(struct_output);
        output.extend(additional_output);
        output
    } else {
        let error_msg = r#"declare_commands_args macro takes 2 arguments.
 First argument is the target type (None if there is no target).
 Second argument is the subcommand type (None if there is no subcommand)."#;
        TokenStream::from(quote! {compile_error!(#error_msg)})
    }
}

#[proc_macro_attribute]
pub fn extend_command_args(args: TokenStream, input: TokenStream) -> TokenStream {
    let args_clone = args.clone();
    let parsed_args =
        parse_macro_input!(args_clone with Punctuated::<Meta, Comma>::parse_terminated);
    if parsed_args.len() != 3 {
        let error_msg = r#"extend_command_args takes three arguments.
 First argument is the type of the base command arguments struct to extend.
 Second argument is the target type (None if there is no target).
 Third argument is the subcommand type (None if there is no subcommand)"#;
        return TokenStream::from(quote! {compile_error!(#error_msg);});
    }
    let mut output: TokenStream = quote! {}.into();
    let (struct_output, additional_output) = generate_command_args_struct(args.clone(), input);
    let tryinto = generate_command_args_tryinto(args, struct_output.clone());
    output.extend(struct_output);
    output.extend(additional_output);
    output.extend(tryinto);
    output
}

// Subcommands
// ===========

fn get_subcommand_variant_map() -> HashMap<&'static str, proc_macro2::TokenStream> {
    HashMap::from([
        (
            "BumpSubCommand",
            quote! {
                #[doc = r"Bump the major version (x.0.0)."]
                Major,
                #[doc = r"Bump the minor version (0.x.0)."]
                Minor,
                #[default]
                #[doc = r"Bump the patch version (0.0.x)."]
                Patch,
            },
        ),
        (
            "CheckSubCommand",
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
            // note: default is manually implemented for this subcommand as the default variant is not a unit variant.
            "CoverageSubCommand",
            quote! {
                #[doc = r"Install grcov and its dependencies."]
                Install,
                #[doc = r"Generate lcov.info file. [default with default debug profile]"]
                Generate(GenerateCmdArgs),
            },
        ),
        (
            "DependenciesSubCommand",
            quote! {
                #[doc = r"Run all dependency checks."]
                #[default]
                All,
                #[doc = r"Run cargo-deny Lint dependency graph to ensure all dependencies meet requirements `<https://crates.io/crates/cargo-deny>`. [default]"]
                Deny,
                #[doc = r"Run cargo-machete to find unused dependencies `<https://crates.io/crates/cargo-machete>`"]
                Unused,
            },
        ),
        (
            "DocSubCommand",
            quote! {
                #[default]
                #[doc = r"Build documentation."]
                Build,
                #[doc = r"Run documentation tests."]
                Tests,
            },
        ),
        (
            "DockerSubCommand",
            quote! {
                #[default]
                #[doc = r"Start docker compose stack."]
                Up,
                #[doc = r"Stop docker compose stack."]
                Down,
            },
        ),
        (
            "FixSubCommand",
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
            "TestSubCommand",
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
            "VulnerabilitiesSubCommand",
            quote! {
                #[default]
                #[doc = r"Run all most useful vulnerability checks. [default]"]
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

fn generate_subcommand_enum(
    subcommand: String,
    enum_name: &syn::Ident,
    original_variants: &Punctuated<Variant, Comma>,
) -> TokenStream {
    let variant_map = get_subcommand_variant_map();
    let output = if let Some(variants) = variant_map.get(subcommand.as_str()) {
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
        // Subcommand not found return no tokens
        quote! {}
    };
    TokenStream::from(output)
}

fn generate_subcomand_tryinto(
    base_subcommand: &syn::Ident,
    subcommand: &syn::Ident,
) -> TokenStream {
    let variant_map = get_subcommand_variant_map();
    // check if variants exist is done by the caller
    let variants = variant_map
        .get(base_subcommand.to_string().as_str())
        .unwrap();
    // parse the variant and look for a default attribute so that we add the default derive if required
    let variants_tokens = TokenStream::from(variants.clone());
    let parsed_variants =
        parse_macro_input!(variants_tokens with Punctuated::<Variant, Comma>::parse_terminated);
    let arms = parsed_variants.iter().map(|v| {
        let variant_ident = &v.ident;
        quote! {
            #subcommand::#variant_ident => Ok(#base_subcommand::#variant_ident),
        }
    });
    let tryinto = quote! {
        impl std::convert::TryInto<#base_subcommand> for #subcommand {
            type Error = anyhow::Error;
            fn try_into(self) -> Result<#base_subcommand, Self::Error> {
                match self {
                    #(#arms)*
                    _ => Err(anyhow::anyhow!("{} target is not supported.", self))
                }
            }
        }
    };
    TokenStream::from(tryinto)
}

#[proc_macro_attribute]
pub fn extend_subcommands(args: TokenStream, input: TokenStream) -> TokenStream {
    let item = parse_macro_input!(input as ItemEnum);
    let args_clone = args.clone();
    let parsed_args =
        parse_macro_input!(args_clone with Punctuated::<Meta, Comma>::parse_terminated);
    if parsed_args.len() != 1 {
        return TokenStream::from(quote! {
            compile_error!("extend_subcommand takes one argument which is the type of the subcommand enum.");
        });
    }
    let base_subcommand = parsed_args.get(0).unwrap();
    let base_subcommand_ident = base_subcommand.path().get_ident().unwrap();
    let base_subcommand_string = base_subcommand_ident.to_string();
    let subcommand_ident = &item.ident;
    let original_variants = &item.variants;

    let variant_map = get_subcommand_variant_map();
    if !variant_map.contains_key(base_subcommand_string.as_str()) {
        let err_msg = format!(
            "Unknown command: {}\nPossible commands are:\n  {}",
            base_subcommand_string,
            variant_map
                .keys()
                .cloned()
                .collect::<Vec<&str>>()
                .join("\n  "),
        );
        return TokenStream::from(quote! { compile_error!(#err_msg); });
    }
    let mut output = generate_subcommand_enum(
        base_subcommand_string.clone(),
        subcommand_ident,
        original_variants,
    );
    output.extend(generate_subcomand_tryinto(
        base_subcommand_ident,
        subcommand_ident,
    ));
    output
}
