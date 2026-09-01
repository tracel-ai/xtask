extern crate proc_macro;
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use std::collections::HashMap;
use syn::{
    ItemEnum, ItemStruct, Meta, Path, Variant, parse_macro_input, punctuated::Punctuated,
    token::Comma,
};

struct CollectedCommandFields {
    fields: Vec<syn::Field>,
    target_type: Option<Meta>,
    subcommand_type: Option<Meta>,
}

type CollectFieldsResult = Result<CollectedCommandFields, TokenStream>;

// Targets
// =======

fn generate_target_enum(item: &ItemEnum) -> TokenStream {
    let enum_name = &item.ident;

    // Remove #[alias(...)] attributes from the actual generated enum
    let original_variants = strip_alias_attributes(&item.variants);

    let output = quote! {
        #[derive(
            strum::EnumString,
            strum::EnumIter,
            Default,
            strum::Display,
            Clone,
            PartialEq,
            clap::ValueEnum,
        )]
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

fn generate_target_tryinto(item: &ItemEnum) -> TokenStream {
    use proc_macro2::Span;
    let item_ident = &item.ident;
    // Base targets we know how to map to tracel_xtask::commands::Target
    const BASES: &[&str] = &["AllPackages", "Crates", "Examples", "Workspace"];
    // groups["Workspace"] = [Workspace, Backend, ...]
    let mut groups: HashMap<&'static str, Vec<syn::Ident>> = HashMap::new();
    for name in BASES {
        groups.insert(name, vec![syn::Ident::new(name, Span::call_site())]);
    }
    // Collect aliases from the original enum
    for variant in &item.variants {
        let from_ident = &variant.ident;
        for attr in &variant.attrs {
            if !attr.path().is_ident("alias") {
                continue;
            }
            // Expect #[alias(xxxxx)]
            let alias_target_path: Path = match attr.parse_args() {
                Ok(p) => p,
                Err(e) => return TokenStream::from(e.to_compile_error()),
            };
            let Some(to_ident) = alias_target_path.get_ident() else {
                let msg = "alias attribute expects a simple identifier, e.g. #[alias(Workspace)]";
                return TokenStream::from(quote! { compile_error!(#msg); });
            };

            let to_name = to_ident.to_string();
            if !BASES.contains(&to_name.as_str()) {
                let msg = format!(
                    "alias can only refer to one of the base targets: {:?}. Found `{}`",
                    BASES, to_name
                );
                return TokenStream::from(quote! { compile_error!(#msg); });
            }

            if let Some(vec) = groups.get_mut(to_name.as_str()) {
                vec.push(from_ident.clone());
            }
        }
    }

    // Build match arms: ExtendedTarget::Workspace | ExtendedTarget::Backend => Target::Workspace
    let mut arms = Vec::new();
    for (base_name, idents) in groups {
        let target_variant = syn::Ident::new(base_name, Span::call_site());
        let mut idents_iter = idents.iter();
        let first = match idents_iter.next() {
            Some(f) => f,
            None => continue,
        };
        // build pattern: Enum::First | Enum::Other | Enum::Another ...
        let mut pattern = quote! { #item_ident::#first };
        for ident in idents_iter {
            pattern = quote! { #pattern | #item_ident::#ident };
        }
        let rhs = quote! { tracel_xtask::commands::Target::#target_variant };
        arms.push(quote! {
            #pattern => Ok(#rhs),
        });
    }

    let tryinto = quote! {
        impl std::convert::TryInto<tracel_xtask::commands::Target> for #item_ident {
            type Error = anyhow::Error;
            fn try_into(self) -> Result<tracel_xtask::commands::Target, Self::Error> {
                match self {
                    #(#arms)*
                    _ => Err(anyhow::anyhow!("{} target is not supported.", self)),
                }
            }
        }
    };

    TokenStream::from(tryinto)
}

#[proc_macro_attribute]
pub fn declare_targets(_args: TokenStream, input: TokenStream) -> TokenStream {
    let item = parse_macro_input!(input as ItemEnum);
    generate_target_enum(&item)
}

#[proc_macro_attribute]
pub fn extend_targets(_args: TokenStream, input: TokenStream) -> TokenStream {
    let item = parse_macro_input!(input as ItemEnum);
    let mut output = generate_target_enum(&item);
    output.extend(generate_target_tryinto(&item));
    output
}

fn strip_alias_attributes(variants: &Punctuated<Variant, Comma>) -> Punctuated<Variant, Comma> {
    let mut cleaned = Punctuated::new();

    for v in variants {
        let mut v2 = v.clone();
        v2.attrs.retain(|attr| !attr.path().is_ident("alias"));
        cleaned.push(v2);
    }

    cleaned
}

// Commands
// ========

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CommandHandler {
    Standard,
    Fix,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CommandMetadata {
    feature: &'static str,
    enabled: bool,
    variant: &'static str,
    module: &'static str,
    args_type: &'static str,
    doc: &'static str,
    alias: Option<&'static str>,
    handler: CommandHandler,
}

macro_rules! command {
    ($feature:literal, $variant:literal, $module:literal, $args_type:literal, $doc:literal) => {
        command!(
            $feature,
            $variant,
            $module,
            $args_type,
            $doc,
            None,
            CommandHandler::Standard
        )
    };
    (
        $feature:literal,
        $variant:literal,
        $module:literal,
        $args_type:literal,
        $doc:literal,
        $alias:expr,
        $handler:expr
    ) => {
        CommandMetadata {
            feature: $feature,
            enabled: cfg!(feature = $feature),
            variant: $variant,
            module: $module,
            args_type: $args_type,
            doc: $doc,
            alias: $alias,
            handler: $handler,
        }
    };
}

// Keep this table in canonical alphabetical command order. Both the generated enum variants and
// their dispatch arms are derived from it so that the two expansions cannot diverge.
const COMMANDS: &[CommandMetadata] = &[
    command!(
        "aws-container",
        "AwsContainer",
        "aws_container",
        "AwsContainerCmdArgs",
        "Manage AWS containers lifecycle, from build to deployment.",
        Some("container"),
        CommandHandler::Standard
    ),
    command!(
        "aws-secrets",
        "AwsSecrets",
        "aws_secrets",
        "AwsSecretsCmdArgs",
        "Manage secrets through AWS secrets manager.",
        Some("secrets"),
        CommandHandler::Standard
    ),
    command!("build", "Build", "build", "BuildCmdArgs", "Build the code."),
    command!(
        "bump",
        "Bump",
        "bump",
        "BumpCmdArgs",
        "Bump the version of all crates to be published."
    ),
    command!(
        "check",
        "Check",
        "check",
        "CheckCmdArgs",
        "Run checks without fixing the issues (use the 'fix' command to auto-fix issues)."
    ),
    command!(
        "clean",
        "Clean",
        "clean",
        "CleanCmdArgs",
        "Clean target directory."
    ),
    command!(
        "compile",
        "Compile",
        "compile",
        "CompileCmdArgs",
        "Compile check the code (does not write binaries to disk)."
    ),
    command!(
        "coverage",
        "Coverage",
        "coverage",
        "CoverageCmdArgs",
        "Install and run coverage tools."
    ),
    command!(
        "dependencies",
        "Dependencies",
        "dependencies",
        "DependenciesCmdArgs",
        "Run the specified dependencies check locally."
    ),
    command!("doc", "Doc", "doc", "DocCmdArgs", "Build documentation."),
    command!(
        "docker-compose",
        "DockerCompose",
        "docker_compose",
        "DockerComposeCmdArgs",
        "Manage docker compose stacks."
    ),
    command!(
        "fix",
        "Fix",
        "fix",
        "FixCmdArgs",
        "Fix issues found with the 'check' command.",
        None,
        CommandHandler::Fix
    ),
    command!(
        "gcp-container",
        "GcpContainer",
        "gcp_container",
        "GcpContainerCmdArgs",
        "Manage GCP containers lifecycle, from build to deployment."
    ),
    command!(
        "gcp-secrets",
        "GcpSecrets",
        "gcp_secrets",
        "GcpSecretsCmdArgs",
        "Manage secrets through GCP secrets manager."
    ),
    command!(
        "host",
        "Host",
        "host",
        "HostCmdArgs",
        "Commands related to an host like connecting, getting info, etc..."
    ),
    command!(
        "icons",
        "Icons",
        "icons",
        "IconsCmdArgs",
        "Generate high-quality PNG and ICO icon files from an SVG.",
        Some("icon"),
        CommandHandler::Standard
    ),
    command!(
        "image",
        "Image",
        "image",
        "ImageCmdArgs",
        "Manage virtual machine images lifecycle, from build to deployment."
    ),
    command!(
        "infra",
        "Infra",
        "infra",
        "InfraCmdArgs",
        "Infrastructure management with terraform."
    ),
    command!(
        "publish",
        "Publish",
        "publish",
        "PublishCmdArgs",
        "Publish a crate to crates.io."
    ),
    command!("test", "Test", "test", "TestCmdArgs", "Runs tests."),
    command!(
        "validate",
        "Validate",
        "validate",
        "ValidateCmdArgs",
        "Validate the code base by running all the relevant checks and tests."
    ),
    command!(
        "vulnerabilities",
        "Vulnerabilities",
        "vulnerabilities",
        "VulnerabilitiesCmdArgs",
        "Run the specified vulnerability check locally. These commands must be called with 'cargo +nightly'."
    ),
];

fn generate_command_variant(command: &CommandMetadata) -> proc_macro2::TokenStream {
    let variant = format_ident!("{}", command.variant);
    let module = format_ident!("{}", command.module);
    let args_type = format_ident!("{}", command.args_type);
    let doc = command.doc;
    let alias = command.alias.map(|alias| {
        quote! {
            #[command(alias = #alias)]
        }
    });

    quote! {
        #alias
        #[doc = #doc]
        #variant(tracel_xtask::commands::#module::#args_type)
    }
}

fn generate_dispatch_function(
    enum_ident: &syn::Ident,
    commands: &[&CommandMetadata],
) -> proc_macro2::TokenStream {
    let arms = commands.iter().map(|command| {
        let variant = format_ident!("{}", command.variant);
        let module = format_ident!("{}", command.module);
        match command.handler {
            CommandHandler::Fix => quote! {
                #enum_ident::#variant(cmd_args) => base_commands::#module::handle_command(cmd_args, env, args.context, None),
            },
            CommandHandler::Standard => quote! {
                #enum_ident::#variant(cmd_args) => base_commands::#module::handle_command(cmd_args, env, args.context),
            },
        }
    });
    quote! {
        fn dispatch_base_commands(args: XtaskArgs<#enum_ident>, env: Environment) -> anyhow::Result<()> {
            match args.command {
                #(#arms)*
                _ => Err(anyhow::anyhow!("Unknown command")),
            }
        }
    }
}

#[proc_macro_attribute]
pub fn base_commands(args: TokenStream, input: TokenStream) -> TokenStream {
    if !args.is_empty() {
        return TokenStream::from(quote! {
            compile_error!("`#[base_commands]` no longer accepts command arguments in v5; enable base commands with `tracel-xtask` Cargo features instead (for example, `features = [\"build\", \"check\"]`).");
        });
    }

    let item = parse_macro_input!(input as ItemEnum);
    let enabled_commands = COMMANDS
        .iter()
        .filter(|command| command.enabled)
        .collect::<Vec<_>>();

    if enabled_commands.is_empty() && item.variants.is_empty() {
        return TokenStream::from(quote! {
            compile_error!("`#[base_commands]` cannot generate an empty command enum; enable at least one `tracel-xtask` command feature or declare a custom command variant.");
        });
    }

    let variants = enabled_commands
        .iter()
        .map(|command| generate_command_variant(command));

    // Generate the xtask commands enum
    let enum_name = &item.ident;
    let other_variants = &item.variants;
    let mut output = TokenStream::from(quote! {
        #[derive(clap::Subcommand, strum::Display)]
        pub enum #enum_name {
            #(#variants,)*
            #other_variants
        }
    });
    output.extend(TokenStream::from(generate_dispatch_function(
        enum_name,
        &enabled_commands,
    )));
    output
}

// Command arguments
// =================

fn get_additional_cmd_args_map() -> HashMap<&'static str, proc_macro2::TokenStream> {
    HashMap::from([
        (
            "BuildCmdArgs",
            quote! {
                #[inherit]
                #[doc = r"Build artifacts in release mode."]
                #[arg(short, long, required = false)]
                pub release: bool,
                #[inherit]
                #[doc = r"Comma-separated list of features to enable."]
                #[arg(
                    short = 'f',
                    long,
                    value_name = "FEATURES,FEATURES,...",
                    value_delimiter = ',',
                    required = false
                )]
                pub features: Vec<String>,
                #[inherit]
                #[doc = r"Define whether to use default features."]
                #[arg(long, default_value_t = false, required = false)]
                pub no_default_features: bool,
            },
        ),
        (
            "CheckCmdArgs",
            quote! {
                #[inherit]
                #[doc = r"Ignore audit errors."]
                #[arg(long = "ignore-audit", required = false)]
                pub ignore_audit: bool,
                #[inherit]
                #[doc = r"Ignore typos errors."]
                #[arg(long = "ignore-typos", required = false)]
                pub ignore_typos: bool,
                #[inherit]
                #[doc = r"Comma-separated list of features to enable."]
                #[arg(
                    short = 'f',
                    long,
                    value_name = "FEATURES,FEATURES,...",
                    value_delimiter = ',',
                    required = false
                )]
                pub features: Vec<String>,
                #[inherit]
                #[doc = r"Define whether to use default features."]
                #[arg(long, default_value_t = false, required = false)]
                pub no_default_features: bool,
            },
        ),
        (
            "DocCmdArgs",
            quote! {
                #[inherit]
                #[doc = r"Comma-separated list of features to enable."]
                #[arg(
                    short = 'f',
                    long,
                    value_name = "FEATURES,FEATURES,...",
                    value_delimiter = ',',
                    required = false
                )]
                #[inherit]
                pub features: Vec<String>,
                #[inherit]
                #[doc = r"Define whether to use default features."]
                #[arg(long, default_value_t = false, required = false)]
                pub no_default_features: bool,
            },
        ),
        (
            "DockerComposeCmdArgs",
            quote! {
                #[inherit]
                #[doc = r"Build images before starting containers."]
                #[arg(short, long, required = false)]
                pub build: bool,
                #[inherit]
                #[doc = r"Project name."]
                #[arg(short, long, default_value = "xtask")]
                pub project: String,
                #[inherit]
                #[doc = r"Space separated list of service subset to start. If empty then launch all the services in the stack."]
                #[arg(short, long, num_args(1..), required = false)]
                pub services: Vec<String>,
            },
        ),
        (
            "FixCmdArgs",
            quote! {
                #[inherit]
                #[doc = r"Comma-separated list of features to enable."]
                #[arg(
                    short = 'f',
                    long,
                    value_name = "FEATURES,FEATURES,...",
                    value_delimiter = ',',
                    required = false
                )]
                pub features: Vec<String>,
                #[inherit]
                #[doc = r"Define whether to use default features."]
                #[arg(long, default_value_t = false, required = false)]
                pub no_default_features: bool,
                #[inherit]
                #[doc = r"If set then bypass confirmation prompt."]
                #[arg(short = 'y', long, global = true)]
                pub yes: bool,
            },
        ),
        (
            "InfraCmdArgs",
            quote! {
                #[inherit]
                #[doc = r"Path where to generate or read the infra configuration."]
                #[arg(long, default_value = "./.tfstates")]
                pub path: PathBuf,
                #[inherit]
                #[doc = r"Path to the Terraform plan file used by `plan` and `apply`."]
                #[arg(long, default_value = "tfplan")]
                pub out: PathBuf,
            },
        ),
        (
            "TestCmdArgs",
            quote! {
                #[inherit]
                #[doc = r"Execute only the test whose name matches the passed string."]
                #[arg(
                    long = "test",
                    value_name = "TEST",
                    required = false
                )]
                pub test: Option<String>,
                #[inherit]
                #[doc = r"Maximum number of parallel test crate compilations."]
                #[arg(
                    long = "compilation-jobs",
                    value_name = "NUMBER OF THREADS",
                    required = false
                )]
                pub jobs: Option<u16>,
                #[inherit]
                #[doc = r"Maximum number of parallel test within a test crate execution."]
                #[arg(
                    long = "test-threads",
                    value_name = "NUMBER OF THREADS",
                    required = false
                )]
                pub threads: Option<u16>,
                #[inherit]
                #[doc = r"Comma-separated list of features to enable during tests."]
                #[arg(
                    long,
                    value_name = "FEATURE,FEATURE,...",
                    value_delimiter = ',',
                    required = false
                )]
                pub features: Option<Vec<String>>,
                #[inherit]
                #[doc = r"If set, ignore default features."]
                #[arg(
                    long = "no-default-features",
                    required = false
                )]
                pub no_default_features: bool,
                #[inherit]
                #[doc = r"Run tests through Miri. If passed without a value, defaults to all. Requires a nightly toolchain."]
                #[arg(
                    long = "miri",
                    value_name = "MODE",
                    value_enum,
                    num_args = 0..2,
                    default_missing_value = "all",
                    required = false
                )]
                pub miri: Option<MiriMode>,
                #[inherit]
                #[doc = r"Force execution of tests no matter the environment (i.e. authorize to execute tests in prod)."]
                #[arg(
                    short = 'f',
                    long = "force",
                    required = false
                )]
                pub force: bool,
                #[inherit]
                #[doc = r"If set, test logs are sent to output."]
                #[arg(long = "nocapture", required = false)]
                pub no_capture: bool,
                #[inherit]
                #[doc = r"Build test in release mode."]
                #[arg(short = 'r', long = "release", required = false)]
                pub release: bool,
            },
        ),
        (
            "ValidateCmdArgs",
            quote! {
                #[inherit]
                #[doc = r"Ignore audit errors."]
                #[arg(long = "ignore-audit", required = false)]
                pub ignore_audit: bool,
                #[inherit]
                #[doc = r"Ignore typos errors."]
                #[arg(long = "ignore-typos", required = false)]
                pub ignore_typos: bool,
                #[inherit]
                #[doc = r"Build in release mode."]
                #[arg(short = 'r', long = "release", required = false)]
                pub release: bool,
                #[inherit]
                #[doc = r"Comma-separated list of features to enable."]
                #[arg(
                    short = 'f',
                    long,
                    value_name = "FEATURES,FEATURES,...",
                    value_delimiter = ',',
                    required = false
                )]
                pub features: Vec<String>,
                #[inherit]
                #[doc = r"Define whether to use default features."]
                #[arg(long, default_value_t = false, required = false)]
                pub no_default_features: bool,
            },
        ),
    ])
}

fn collect_generated_command_fields(
    args: &Punctuated<Meta, Comma>,
    item: &ItemStruct,
) -> CollectFieldsResult {
    let struct_name = &item.ident;

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
        return Err(TokenStream::from(quote! {
            compile_error!("Error expanding macro.");
        }));
    }

    let target_fields_tokens = if let Some(target) = target_type.clone() {
        quote! {
            #[doc = r"The target on which executing the command."]
            #[arg(short, long, value_enum, default_value_t = #target::default())]
            pub target: #target,

            #[inherit]
            #[doc = r"Comma-separated list of excluded crates."]
            #[arg(
                short = 'x',
                long,
                value_name = "CRATE,CRATE,...",
                value_delimiter = ',',
                required = false
            )]
            pub exclude: Vec<String>,

            #[inherit]
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

    let additional_fields_tokens = match additional_cmd_args_map.get(base_command_type.as_str()) {
        Some(fields) => fields.clone(),
        None => quote! {},
    };

    let subcommand_field_tokens = if let Some(subcommand) = subcommand_type.clone() {
        quote! {
            #[command(subcommand)]
            pub command: Option<#subcommand>,
        }
    } else {
        quote! {}
    };

    let mut fields = Vec::new();

    match parse_named_fields(target_fields_tokens) {
        Ok(parsed) => fields.extend(parsed),
        Err(e) => return Err(TokenStream::from(e.to_compile_error())),
    }

    match parse_named_fields(additional_fields_tokens) {
        Ok(parsed) => fields.extend(parsed),
        Err(e) => return Err(TokenStream::from(e.to_compile_error())),
    }

    match parse_named_fields(subcommand_field_tokens) {
        Ok(parsed) => fields.extend(parsed),
        Err(e) => return Err(TokenStream::from(e.to_compile_error())),
    }

    fields.extend(item.fields.iter().cloned());

    Ok(CollectedCommandFields {
        fields,
        target_type,
        subcommand_type,
    })
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

    let CollectedCommandFields {
        fields,
        target_type: _target_type,
        subcommand_type,
    } = match collect_generated_command_fields(&args, &item) {
        Ok(v) => v,
        Err(e) => return (e, TokenStream::new()),
    };

    let emitted_fields = fields.iter().map(|f| {
        let f = strip_internal_field_attrs(f);
        let attrs = &f.attrs;
        let vis = &f.vis;
        let ident = &f.ident;
        let ty = &f.ty;
        quote! {
            #(#attrs)*
            #vis #ident: #ty
        }
    });

    let struct_output = TokenStream::from(quote! {
        #[derive(clap::Args, Clone)]
        pub struct #struct_name {
            #(#emitted_fields,)*
        }
    });

    let (subcommand_impl, maybe_subcommand_enum) = if let Some(subcommand) = subcommand_type {
        let subcommand_impl = quote! {
            impl #struct_name {
                pub fn get_command(&self) -> #subcommand {
                    self.command.clone().unwrap_or_default()
                }
            }
        };

        let maybe_subcommand_enum = if args.len() == 2 {
            let subcommand_ident = subcommand.path().get_ident().unwrap();
            let subcommand_string = subcommand_ident.to_string();
            let original_variants = Punctuated::<Variant, Comma>::new();
            generate_subcommand_enum(subcommand_string, subcommand_ident, &original_variants)
        } else {
            TokenStream::new()
        };

        (TokenStream::from(subcommand_impl), maybe_subcommand_enum)
    } else {
        (TokenStream::new(), TokenStream::new())
    };

    let mut additional_output = TokenStream::new();
    additional_output.extend(subcommand_impl);
    additional_output.extend(maybe_subcommand_enum);

    (struct_output, additional_output)
}

fn generate_command_args_tryinto(args: TokenStream, input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args with Punctuated::<Meta, Comma>::parse_terminated);
    let base_type = args.get(0).unwrap();
    let base_type_string = base_type.path().get_ident().unwrap().to_string();

    let item = match syn::parse::<ItemStruct>(input) {
        Ok(data) => data,
        Err(e) => return TokenStream::from(e.to_compile_error()),
    };
    let item_ident = &item.ident;

    let CollectedCommandFields {
        fields,
        target_type: _target_type,
        subcommand_type: _subcommand_type,
    } = match collect_generated_command_fields(&args, &item) {
        Ok(v) => v,
        Err(e) => return e,
    };

    let has_target = fields.iter().any(|f| {
        if let Some(ident) = &f.ident {
            *ident == "target"
        } else {
            false
        }
    });

    // Only forward `command` if:
    // 1. the generated/extended struct has a `command` field
    // 2. the base command type itself supports subcommands
    let subcommand_variant_map = get_subcommand_variant_map();
    let base_subcommand_type_string = base_type_string.replace("CmdArgs", "SubCommand");
    let base_has_subcommand =
        subcommand_variant_map.contains_key(base_subcommand_type_string.as_str());

    let has_command_field = fields.iter().any(|f| {
        if let Some(ident) = &f.ident {
            *ident == "command"
        } else {
            false
        }
    });

    let has_subcommand = base_has_subcommand && has_command_field;

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

    let forwarded_fields: Vec<_> = fields
        .iter()
        .filter(|f| has_inherit_attr(f))
        .filter_map(|f| {
            f.ident.as_ref().map(|ident| {
                quote! { #ident: self.#ident, }
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
                    #(#forwarded_fields)*
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
    let input_clone = input.clone();
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
    let tryinto = generate_command_args_tryinto(args, input_clone);
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
            "AwsContainerSubCommand",
            quote! {
                #[doc = r"Build a container."]
                Build(AwsContainerBuildSubCmdArgs),
                #[doc = r"Start a terminal session on a container host instance."]
                Host(AwsContainerHostSubCmdArgs),
                #[doc = r"Show current latest and rollback images in registry."]
                List(AwsContainerListSubCmdArgs),
                #[doc = r"Show Cloudwatch logs of the container."]
                Logs(AwsContainerLogsSubCmdArgs),
                #[doc = r"Pull a container from a registry."]
                Pull(AwsContainerPullSubCmdArgs),
                #[doc = r"Push a container to a registry."]
                Push(AwsContainerPushSubCmdArgs),
                #[doc = r"Promote a pushed container to latest."]
                Promote(AwsContainerPromoteSubCmdArgs),
                #[doc = r"Rollback previously released container to latest."]
                Rollback(AwsContainerRollbackSubCmdArgs),
                #[doc = r"Rollout last promoted container."]
                Rollout(AwsContainerRolloutSubCmdArgs),
                #[doc = r"Run a local container."]
                Run(AwsContainerRunSubCmdArgs),
            },
        ),
        (
            "AwsSecretsSubCommand",
            quote! {
                #[doc = r"Create an empty secret (metadata only, no version)."]
                Create(AwsSecretsCreateSubCmdArgs),
                #[doc = r"Copy a secret value from one secret ID to another in the same region."]
                Copy(AwsSecretsCopySubCmdArgs),
                #[doc = r"Fetch latest version of a secret and open the default editor to edit it."]
                Edit(AwsSecretsEditSubCmdArgs),
                #[doc = r"Fetch the secrets and write an environment file to a specified path."]
                EnvFile(AwsSecretsEnvFileSubCmdArgs),
                #[doc = r"List all versions of a secret."]
                List(AwsSecretsListSubCmdArgs),
                #[doc = r"Push new key-value pairs to existing secrets."]
                Push(AwsSecretsPushSubCmdArgs),
                #[doc = r"Show the latest version of a secret."]
                View(AwsSecretsViewSubCmdArgs),
            },
        ),
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
                Generate(GenerateSubCmdArgs),
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
                #[doc = r"Run cargo update to resolve against the latest version of the dependencies."]
                Update,
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
            "DockerComposeSubCommand",
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
                #[doc = r"Run lint and format."]
                Code,
                #[doc = r"Run format command and fix formatting."]
                Format,
                #[doc = r"Run lint command and fix issues."]
                Lint,
                #[doc = r"Find typos in source code and fix them."]
                Typos,
            },
        ),
        (
            "GcpContainerSubCommand",
            quote! {
                #[doc = r"Build a GCP container."]
                Build(GcpContainerBuildSubCmdArgs),
                #[doc = r"Show current latest and rollback images in Artifact Registry."]
                List(GcpContainerListSubCmdArgs),
                #[doc = r"Pull a container from Artifact Registry."]
                Pull(GcpContainerPullSubCmdArgs),
                #[doc = r"Push a container to Artifact Registry."]
                Push(GcpContainerPushSubCmdArgs),
                #[doc = r"Promote a pushed container to the environment tag."]
                Promote(GcpContainerPromoteSubCmdArgs),
                #[doc = r"Rollback previously released container to the environment tag."]
                Rollback(GcpContainerRollbackSubCmdArgs),
                #[doc = r"Rollout last promoted container through a GCP Managed Instance Group."]
                Rollout(GcpContainerRolloutSubCmdArgs),
                #[doc = r"Run a local container."]
                Run(GcpContainerRunSubCmdArgs),
            },
        ),
        (
            "GcpSecretsSubCommand",
            quote! {
                #[doc = r"Create an empty secret (metadata only, no version)."]
                Create(GcpSecretsCreateSubCmdArgs),
                #[doc = r"Copy a secret value from one secret ID to another in the same region."]
                Copy(GcpSecretsCopySubCmdArgs),
                #[doc = r"Fetch latest version of a secret and open the default editor to edit it."]
                Edit(GcpSecretsEditSubCmdArgs),
                #[doc = r"Fetch the secrets and write an environment file to a specified path."]
                EnvFile(GcpSecretsEnvFileSubCmdArgs),
                #[doc = r"List all versions of a secret."]
                List(GcpSecretsListSubCmdArgs),
                #[doc = r"Push new key-value pairs to existing secrets."]
                Push(GcpSecretsPushSubCmdArgs),
                #[doc = r"Show the latest version of a secret."]
                View(GcpSecretsViewSubCmdArgs),
            },
        ),
        (
            "HostSubCommand",
            quote! {
                #[doc = r"Connect to the host."]
                Connect(HostConnectSubCmdArgs),
                #[doc = r"Fetch the private IP of the host if any."]
                PrivateIp(HostPrivateIpSubCmdArgs),
            },
        ),
        (
            "ImageSubCommand",
            quote! {
                #[doc = r"Build virtual machine images from Terraform-managed baker instances."]
                Build(ImageBuildSubCmdArgs),
                #[doc = r"Clean obsolete virtual machine images."]
                Clean(ImageCleanSubCmdArgs),
                #[doc = r"Start a terminal session on an image baker instance."]
                Host(ImageHostSubCmdArgs),
                #[doc = r"Show current latest and rollback virtual machine images."]
                List(ImageListSubCmdArgs),
                #[doc = r"Promote a built virtual machine image to latest."]
                Promote(ImagePromoteSubCmdArgs),
                #[doc = r"Rollback previously promoted virtual machine image to latest."]
                Rollback(ImageRollbackSubCmdArgs),
                #[doc = r"Rollout last promoted virtual machine image."]
                Rollout(ImageRolloutSubCmdArgs),
            },
        ),
        (
            "InfraSubCommand",
            quote! {
                #[doc = r"Apply infra changes."]
                Apply(InfraApplySubCmdArgs),
                #[doc = r"Create a destroy plan."]
                Destroy(InfraDestroySubCmdArgs),
                #[doc = r"Initialize terraform."]
                Init,
                #[doc = r"Install the locked version of terraform or the latest version if there is no lock file yet."]
                Install(InfraInstallSubCmdArgs),
                #[doc = r"List all the installed versions of terraform."]
                List,
                #[doc = r"List outputs of a terraform state."]
                Output(InfraOutputSubCmdArgs),
                #[default]
                #[doc = r"Create a plan for infra changes."]
                Plan,
                #[doc = r"Call terraform providers command."]
                Providers(InfraProvidersSubCmdArgs),
                #[doc = r"Uninstall the locked version of terraform."]
                Uninstall(InfraUninstallSubCmdArgs),
                #[doc = r"Update locked version of terraform to latest."]
                Update,
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

// Utils
// =====

fn has_inherit_attr(field: &syn::Field) -> bool {
    field
        .attrs
        .iter()
        .any(|attr| attr.path().is_ident("inherit"))
}

fn strip_internal_field_attrs(field: &syn::Field) -> syn::Field {
    let mut field = field.clone();
    field.attrs.retain(|attr| !attr.path().is_ident("inherit"));
    field
}

fn parse_named_fields(tokens: proc_macro2::TokenStream) -> Result<Vec<syn::Field>, syn::Error> {
    if tokens.is_empty() {
        return Ok(Vec::new());
    }

    let dummy: ItemStruct = syn::parse2(quote! {
        struct __XtaskGeneratedFields {
            #tokens
        }
    })?;

    match dummy.fields {
        syn::Fields::Named(fields) => Ok(fields.named.into_iter().collect()),
        _ => unreachable!("dummy struct should always have named fields"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_metadata_is_complete_and_alphabetical() {
        assert_eq!(COMMANDS.len(), 22);

        let features = COMMANDS
            .iter()
            .map(|command| command.feature)
            .collect::<Vec<_>>();
        let mut sorted_features = features.clone();
        sorted_features.sort_unstable();
        sorted_features.dedup();

        assert_eq!(features, sorted_features);
    }

    #[test]
    fn dispatch_uses_the_input_enum_and_fix_signature() {
        let enum_ident = format_ident!("ProjectCommand");
        let fix = COMMANDS
            .iter()
            .find(|command| command.feature == "fix")
            .unwrap();
        let output = generate_dispatch_function(&enum_ident, &[fix]).to_string();

        assert!(output.contains("XtaskArgs < ProjectCommand >"));
        assert!(output.contains("ProjectCommand :: Fix"));
        assert!(output.contains("args . context , None"));
    }

    #[test]
    fn command_variants_retain_aliases_and_argument_types() {
        let aws_container = COMMANDS
            .iter()
            .find(|command| command.feature == "aws-container")
            .unwrap();
        let output = generate_command_variant(aws_container).to_string();

        assert!(output.contains("alias = \"container\""));
        assert!(output.contains("aws_container :: AwsContainerCmdArgs"));
    }
}
