use thiserror::Error;
use std::collections::HashMap;
use std::path::PathBuf;

enum BasicError {
    UnsupportedConfigVersion,
    InvalidConfigOption,
    UnclosedTagStart,
    NotConfigVersion,
    NonUnicodeComment,
    InvalidBool,
    #[allow(dead_code)]
    InvalidPathEncoding,
    NonUnicodeTag,
}

#[derive(Clone, Debug, Error)]
pub enum TextError {
    #[error("unsupported config version on line {0}")]
    UnsupportedConfigVersion(usize),
    #[error("invalid config option on line {0}")]
    InvalidConfigOption(usize),
    #[error("unclosed tag start on line {0}")]
    UnclosedTagStart(usize),
    #[error("first config option isn't config-version on line {0}")]
    NotConfigVersion(usize),
    #[error("non-unicode comment on line {0}")]
    NonUnicodeComment(usize),
    #[error("invalid boolean option value on line {0}")]
    InvalidBool(usize),
    #[error("invalid path encoding on line {0}")]
    InvalidPathEncoding(usize),
    #[error("non-unicode tag on line {0}")]
    NonUnicodeTag(usize),
}

impl TextError {
    fn new(err: BasicError, line: usize) -> Self {
        // start with 1
        let line = line + 1;
        match err {
            BasicError::UnclosedTagStart => Self::UnclosedTagStart(line),
            BasicError::NotConfigVersion => Self::NotConfigVersion(line),
            BasicError::InvalidConfigOption => Self::InvalidConfigOption(line),
            BasicError::UnsupportedConfigVersion => Self::UnsupportedConfigVersion(line),
            BasicError::NonUnicodeComment => Self::NonUnicodeComment(line),
            BasicError::InvalidBool => Self::InvalidBool(line),
            BasicError::InvalidPathEncoding => Self::InvalidPathEncoding(line),
            BasicError::NonUnicodeTag => Self::NonUnicodeTag(line),
        }
    }
}

#[derive(Clone, Debug, Error)]
pub enum Error {
    #[error("{0}")]
    Text(#[from] #[source] TextError),
}

#[derive(Debug, PartialEq, Eq)]
enum ConfigOption {
    ConfigVersion0,
    FollowMounts(bool),
    FollowLinks(bool),
    BasePath(PathBuf),
}

#[derive(Debug, PartialEq, Eq)]
enum TagLine {
    TagStart(String),
    Rule(PathBuf),
    Include(String),
}

#[derive(Debug, PartialEq, Eq)]
struct Line<T> {
    inner: Option<T>,
    comment: Option<String>,
}

impl<T> Line<T> {
    fn new<S: AsRef<[u8]>>(inner: Option<T>, comment: Option<S>) -> Result<Self, BasicError> {
        let comment = comment
            .map(|s| {
                String::from_utf8(s.as_ref().to_owned())
            })
            .transpose()
            .map_err(|_| BasicError::NonUnicodeComment)?;
        Ok(Self {
            inner,
            comment,
        })
    }
}

#[derive(Debug)]
struct ConfigText {
    options: Vec<Line<ConfigOption>>,
    rules: Vec<Line<TagLine>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum Rule {
    Exact(PathBuf),
    Dir(PathBuf),
    Suffix(PathBuf, PathBuf),
    File(PathBuf),
    /// symlink at path a to b
    SymLink(PathBuf, Option<PathBuf>),
    // Dir with only symlinks (at path a to b)
    SymLinkDir(PathBuf, Option<PathBuf>),
    MountPoint(PathBuf),
    Tag(String),
}

#[derive(Clone, Default, Debug)]
pub struct Tag {
    pub rules: Vec<Rule>,
}

#[derive(Debug, Default)]
pub struct Config {
    pub follow_mounts: bool,
    pub follow_links: bool,
    pub base_path: PathBuf,
    pub tags: HashMap<String, Tag>,
}

fn parse_path(text: &[u8]) -> Result<PathBuf, BasicError> {
    #[cfg(unix)]
    {
        use std::ffi::*;
        use std::os::unix::ffi::*;
        Ok(PathBuf::from(OsStr::from_bytes(text)))
    }
    #[cfg(not(unix))]
    {
        // Default to utf-8
        Ok(PathBuf::from(String::from_utf8(text.to_owned()).map_err(|err| BasicError::InvalidPathEncoding)?))
    }
}

fn parse_config(text: &[u8]) -> Result<ConfigOption, BasicError> {
    return if text.starts_with(b"config-version=") {
        if text == b"config-version=0" {
            Ok(ConfigOption::ConfigVersion0)
        } else {
            Err(BasicError::UnsupportedConfigVersion)
        }
    } else if text.starts_with(b"follow-mounts=") {
        if text == b"follow-mounts=false" {
            Ok(ConfigOption::FollowMounts(false))
        } else if text == b"follow-mounts=true" {
            Ok(ConfigOption::FollowMounts(true))
        } else {
            Err(BasicError::InvalidBool)
        }
    } else if text.starts_with(b"follow-links=") {
        if text == b"follow-links=false" {
            Ok(ConfigOption::FollowLinks(false))
        } else if text == b"follow-links=true" {
            Ok(ConfigOption::FollowLinks(true))
        } else {
            Err(BasicError::InvalidBool)
        }
    } else if let Some(path) = text.strip_prefix(b"base-path=") {
        Ok(ConfigOption::BasePath(parse_path(path)?))
    } else {
        Err(BasicError::InvalidConfigOption)
    };
}

fn parse_tag_line(text: &[u8]) -> Result<TagLine, BasicError> {
    if text.first() == Some(&b'[') {
        if text.last() != Some(&b']') {
            Err(BasicError::UnclosedTagStart)
        } else {
            Ok(TagLine::TagStart(
                String::from_utf8(text[1..text.len() - 1].to_owned())
                    .map_err(|_| BasicError::NonUnicodeTag)?
            ))
        }
    } else if text.first() == Some(&b'@') {
        Ok(TagLine::Include(String::from_utf8(text[1..].to_owned()).map_err(|_| BasicError::NonUnicodeTag)?))
    } else {
        parse_path(text).map(TagLine::Rule)
    }
}

fn parse_text(text: &[u8]) -> Result<ConfigText, TextError> {
    enum Stage {
        Version,
        Config,
        Tags,
    }
    let mut options = vec![];
    let mut rules = vec![];
    let mut stage = Stage::Version;
    for (ln, line) in text.split(|x| *x == b'\n').enumerate() {
        let (content, comment) = if line.is_empty() {
            (None, None)
        } else if line.first() == Some(&b'#') {
            (None, Some(&line[1..]))
        } else {
            let mut comment_start = None;
            for (i, window) in line.windows(2).enumerate() {
                if window[1] == b'#' && window[0].is_ascii_whitespace() {
                    comment_start = Some(i);
                    break
                }
            }
            if let Some(comment_start) = comment_start {
                if comment_start == 0 {
                    (None, Some(&line[2..]))
                } else {
                    (Some(&line[..comment_start]), Some(&line[comment_start + 2..]))
                }
            } else {
                (Some(line), None)
            }
        };
        let ferr = move |err| TextError::new(err, ln);
        'goto: loop {
            match stage {
                Stage::Version => {
                    if let Some(content) = content {
                        let config = parse_config(content).map_err(ferr)?;
                        if config != ConfigOption::ConfigVersion0 {
                            return Err(ferr(BasicError::NotConfigVersion))
                        }
                        stage = Stage::Config;
                        options.push(Line::new(Some(config), comment).map_err(ferr)?);
                    } else {
                        options.push(Line::new(None, comment).map_err(ferr)?);
                    }
                }
                Stage::Config => {
                    if line.first() == Some(&b'[') {
                        stage = Stage::Tags;
                        continue 'goto
                    }
                    options.push(Line::new(content.map(parse_config).transpose().map_err(ferr)?, comment).map_err(ferr)?); 
                }
                Stage::Tags => {
                    rules.push(Line::new(content.map(parse_tag_line).transpose().map_err(ferr)?, comment).map_err(ferr)?);
                }
            }
            break 'goto
        }
    }
    Ok(ConfigText {
        options,
        rules,
    })
}

pub fn parse(text: &[u8]) -> Result<Config, Error> {
    let text = parse_text(text)?;
    let mut ret = Config::default();
    for option in text.options {
        match option.inner {
            Some(ConfigOption::ConfigVersion0) => {}
            Some(ConfigOption::BasePath(path)) => ret.base_path = path,
            Some(ConfigOption::FollowMounts(val)) => ret.follow_mounts = val,
            Some(ConfigOption::FollowLinks(val)) => ret.follow_links = val,
            None => {}
        }
    }
    let mut tag_name = None;
    let mut tag = Tag::default();
    for tag_line in text.rules {
        match tag_line.inner {
            Some(TagLine::TagStart(name)) => {
                if let Some(tag_name) = tag_name {
                    ret.tags.insert(tag_name, tag);
                    tag = Tag::default();
                }
                tag_name = Some(name);
            },
            Some(TagLine::Include(name)) => {
                tag.rules.push(if let Some(name) = name.strip_prefix("exact;") {
                        Rule::Exact(name.into())
                } else if let Some(name) = name.strip_prefix("symlink;") {
                    if let Some((a, b)) = name.split_once(";") {
                        Rule::SymLink(a.into(), Some(b.into()))
                    } else {
                        Rule::SymLink(name.into(), None)
                    }
                } else if let Some(name) = name.strip_prefix("symlink-dir;") {
                    if let Some((a, b)) = name.split_once(";") {
                        Rule::SymLinkDir(a.into(), Some(b.into()))
                    } else {
                        Rule::SymLinkDir(name.into(), None)
                    }
                } else if let Some(name) = name.strip_prefix("mount-point;") {
                    Rule::MountPoint(name.into())
                } else {
                    Rule::Tag(name)
                });
            }
            Some(TagLine::Rule(rule)) => {
                if let Some((a, b)) = rule.as_os_str().to_string_lossy().split_once("/**/") {
                    tag.rules.push(Rule::Suffix(a.into(), b.into()));
                } else if rule.as_os_str().to_string_lossy().ends_with('/') {
                    tag.rules.push(Rule::Dir(rule));
                } else {
                    tag.rules.push(Rule::File(rule));
                }
            }
            None => {}
        }
    }
    if let Some(tag_name) = tag_name {
        ret.tags.insert(tag_name, tag);
    }
    Ok(ret)
}
