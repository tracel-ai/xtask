#[macro_export]
macro_rules! declare_target {
    ($name:ident $(, $($variant:ident),*)?) => {
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
    }
}
