const SUBREPO_EMOJIS: &[(&str, &str)] = &[
    ("admin", "⚙️"),
    ("api", "🔌"),
    ("app", "📱"),
    ("application", "📱"),
    ("backend", "🧠"),
    ("cd", "🚀"),
    ("ci", "🤖"),
    ("console", "🖥️"),
    ("data", "🗄️"),
    ("db", "🗄️"),
    ("dev", "👨‍💻"),
    ("finance", "💰"),
    ("frontend", "🖥️"),
    ("gallery", "🖼️"),
    ("infra", "🏗️"),
    ("ledger", "💰"),
    ("monitor", "🚨"),
    ("money", "💰"),
    ("ops", "🧰"),
    ("platform", "🏗️"),
    ("plugin", "🧩"),
    ("server", "🛰️"),
    ("stack", "🧱"),
    ("tool", "🛠️"),
    ("ui", "🎨"),
    ("web", "🌐"),
];

pub fn emoji_for_subrepo(name: &str) -> Option<&'static str> {
    let n = name.to_ascii_lowercase();
    let mut best_match: Option<(&str, &str)> = None;

    for (needle, emoji) in SUBREPO_EMOJIS {
        if n.contains(needle)
            && best_match
                .map(|(best_needle, _)| needle.len() > best_needle.len())
                .unwrap_or(true)
        {
            best_match = Some((*needle, *emoji));
        }
    }

    best_match.map(|(_, emoji)| emoji)
}

pub fn format_repo_label(name: &str) -> String {
    match emoji_for_subrepo(name) {
        Some(e) => format!("{e} {name}"),
        None => name.to_string(),
    }
}

pub fn print_run_header(label: &str) {
    let width = 78;
    let header = label.to_string();
    let sep = "─".repeat(width);
    eprintln!("{sep}");
    eprintln!(" {header}");
    eprintln!("{sep}");
}
