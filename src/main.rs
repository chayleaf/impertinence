use clap::Parser;
use std::collections::HashSet;
use std::ffi::OsString;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

mod config;

#[derive(Debug, Parser)]
enum Commands {
    Or {
        config: PathBuf,
        first_rule: String,
        other_rules: Vec<String>,
    },
    Nor {
        config: PathBuf,
        first_rule: String,
        other_rules: Vec<String>,
    },
    And {
        config: PathBuf,
        first_rule: String,
        second_rule: String,
        other_rules: Vec<String>,
    },
}

impl Commands {
    fn config(&self) -> &Path {
        match self {
            Self::Or { config, .. } => config,
            Self::Nor { config, .. } => config,
            Self::And { config, .. } => config,
        }
    }
    fn rules(&self) -> Vec<String> {
        match self {
            Self::Or { first_rule, other_rules, .. } => {
                let mut rules = Vec::with_capacity(other_rules.len() + 1);
                rules.push(first_rule.clone());
                rules.extend(other_rules.iter().cloned());
                rules
            }
            Self::Nor { first_rule, other_rules, .. } => {
                let mut rules = Vec::with_capacity(other_rules.len() + 1);
                rules.push(first_rule.clone());
                rules.extend(other_rules.iter().cloned());
                rules
            }
            Self::And { first_rule, second_rule, other_rules, .. } => {
                let mut rules = Vec::with_capacity(other_rules.len() + 2);
                rules.push(first_rule.clone());
                rules.push(second_rule.clone());
                rules.extend(other_rules.iter().cloned());
                rules
            }
        }
    }
}


fn is_symlink_dir_to(path: &Path, target: &Path) -> bool {
    /*if path.starts_with("/home/user/.local/share/Steam") {
        return false
    }
    if !path.starts_with("/home/user/.config") && !path.starts_with("/home/user/.local") && !path.starts_with("/home/user/.nix-defexpr") && !path.starts_with("/home/user/.icons") {
        return false
    }*/
    if let Ok(w) = path.read_dir() {
        // let mut has_symlinks = false;
        for w in w {
            match w {
                Ok(w) => {
                    let metadata = if let Ok(meta) = w.metadata() { meta } else { return false };
                    if (metadata.is_symlink() && is_symlink_to(&w.path(), target))
                        || (metadata.is_dir() && is_symlink_dir_to(&w.path(), target)) {
                        // has_symlinks = true;
                    } else {
                        return false
                    }
                }
                Err(_) => return false,
            }
        }
        true
    } else {
        false
    }
}


fn is_symlink_to(path: &Path, target: &Path) -> bool {
    path.read_link().ok().filter(|x| x.starts_with(target)).is_some()
}

#[derive(Clone, Debug)]
enum Rule {
    Plain(String),
    MountPoint(String),
    SymLink(String, Option<PathBuf>),
    SymLinkDir(String, Option<PathBuf>),
    Suffix(String, PathBuf),
    Exact(String, PathBuf),
}

fn add_rules_to_tree(config: &config::Config, key: &str, rule_name: &str, tree: &mut rule_tree::RulesTree<OsString, Rule>) {
    let mut rules = vec![rule_name.to_owned()];
    let mut added = HashSet::new();
    while let Some(rule) = rules.pop() {
        if added.contains(&rule) {
            continue;
        }
        added.insert(rule.clone());
        for rule in &config.tags.get(&rule).unwrap().rules {
            match rule {
                config::Rule::Suffix(path, sfx) => {
                    tree.add_rule(path, key, rule_tree::TreeRule::prepend(Rule::Suffix(rule_name.to_owned(), sfx.to_owned())));
                }
                config::Rule::Dir(dir) => {
                    tree.add_rule(dir, key, rule_tree::TreeRule::overwrite(Rule::Plain(rule_name.to_owned())));
                },
                config::Rule::File(file) => {
                    tree.add_rule(file, key, rule_tree::TreeRule::overwrite(Rule::Plain(rule_name.to_owned())));
                },
                config::Rule::Exact(name) => {
                    tree.add_rule(name, key, rule_tree::TreeRule::prepend(Rule::Exact(rule_name.to_owned(), name.clone())));
                }
                config::Rule::MountPoint(path) => {
                    tree.add_rule(path, key, rule_tree::TreeRule::prepend(Rule::MountPoint(rule_name.to_owned())));
                },
                config::Rule::SymLink(path, target) => {
                    tree.add_rule(path, key, rule_tree::TreeRule::prepend(Rule::SymLink(rule_name.to_owned(), target.clone())));
                }
                config::Rule::SymLinkDir(path, target) => {
                    tree.add_rule(path, key, rule_tree::TreeRule::prepend(Rule::SymLinkDir(rule_name.to_owned(), target.clone())));
                }
                config::Rule::Tag(name) => {
                    rules.push(name.to_owned());
                }
            }
        }
    }
}

fn main() {
    let args = Commands::parse();
    let conf_path = args.config();
    let mut config = Vec::new();
    let mut file = fs::File::open(conf_path).unwrap();
    file.read_to_end(&mut config).unwrap();
    let config = config::parse(&config).unwrap();
    let argrules = args.rules();
    let walker = walkdir::WalkDir::new(&config.base_path).same_file_system(config.follow_mounts).follow_links(config.follow_links);

    let mut rules = rule_tree::RulesTree::new();
    let count = if matches!(args, Commands::And { .. }) { argrules.len() } else { 1 };
    for (i, rule) in argrules.into_iter().enumerate() {
        if matches!(args, Commands::And { .. }) {
            add_rules_to_tree(&config, &format!("rule{}", i), &rule, &mut rules);
        } else {
            add_rules_to_tree(&config, "rule0", &rule, &mut rules);
        }
    }

    let mut add_rules = vec![];
    for f in walker.into_iter().flatten() {
        let Ok(path) = f.path().strip_prefix(&config.base_path) else {
            continue
        };
        let mut matches = Vec::with_capacity(count);
        for i in 0..count {
            let r = rules.get_rules(path, &format!("rule{i}"));
            add_rules.clear();
            let mut m = false;
            for rule in r {
                match rule.get() {
                    Rule::Plain(_) => {
                        m = true;
                    }
                    Rule::SymLink(_, target) => {
                        if f.path_is_symlink() && target.as_ref().map(|target| is_symlink_to(f.path(), target)).unwrap_or(true) {
                            m = true;
                        }
                    }
                    Rule::SymLinkDir(rule_name, target) => {
                        if !f.path_is_symlink() && f.file_type().is_dir() && target.as_ref().map(|target| is_symlink_dir_to(f.path(), target)).unwrap_or(true) {
                            add_rules.push(rule_tree::TreeRule::overwrite(Rule::Plain(rule_name.to_owned())));
                            m = true;
                        }
                    }
                    Rule::Suffix(rule_name, sfx) => {
                        if f.path().ends_with(sfx) {
                            add_rules.push(rule_tree::TreeRule::overwrite(Rule::Plain(rule_name.to_owned())));
                            m = true;
                        }
                    }
                    Rule::Exact(_, path2) => {
                        if path == path2 {
                            m = true;
                        }
                    }
                    Rule::MountPoint(_) => {
                        todo!()
                    }
                }
            }
            for rule in add_rules.drain(..) {
                rules.add_rule(path, &format!("rule{i}"), rule);
            }
            matches.push(m);
        }
        let m = match args {
            Commands::Or { .. } | Commands::And { .. } => {
                matches.iter().any(|x| *x)
            }
            Commands::Nor { .. } => {
                matches.iter().all(|x| !x)
            }
        };
        if m {
            println!("{}", path.display());
        }
    }
}
