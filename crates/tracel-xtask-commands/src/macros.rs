#[macro_export]
macro_rules! declare_target {
    ($name:ident, no_try_into $(, $($variant:ident),*)?) => {
        #[derive(EnumString, EnumIter, Default, Display, Clone, PartialEq, ValueEnum)]
        #[strum(serialize_all = "lowercase")]
        pub enum $name {
            AllPackages,
            Crates,
            Examples,
            #[default]
            Workspace,
            $($($variant),*)?
        }
    };
    ($name:ident $(, $($variant:ident),*)?) => {
        declare_target!($name, no_try_into $(, $($variant),*)?);

        impl std::convert::TryInto<Target> for $name {
            type Error = anyhow::Error;
            fn try_into(self) -> Result<Target, Self::Error> {
                match self {
                    $name::AllPackages => Ok(Target::AllPackages),
                    $name::Crates => Ok(Target::Crates),
                    $name::Examples => Ok(Target::Examples),
                    $name::Workspace => Ok(Target::Workspace),
                    _ => Err(anyhow::anyhow!("{} target is not supported.", self))
                }
            }
        }
    }
}
