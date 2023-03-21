# Impertinence - a file... manager?

This program isn't a file manager, however it is a program to manage
your files. To put it simply, it allows you to apply rules to your files
so that you can perform commands on those files.

It can notify you of new files that existing rules don't match. This
helps you keep track of *all* of your files, removing useless files like
cache from programs you no longer use.

Example: if you want to create two groups of files, bind mounting each
of them to the correct place depending on the group, you could run
`cargo run -- or --dump rule.cfg <group1> <group2>`. This will generate
a bunch of lines in the format `groupname:path`, giving you exactly the
paths you need to mount.

Example: if you just want to keep track of all of your files, do `cargo
run -- nor rule.cfg <group>`, where `group` contains what are supposedly
all of your files.

## Configuration format

```
# Comment format:
# Anything after the `#` symbol is ignored, as long as
# the `#` symbol follows either an ascii whitespace or the line start
this#is#not#a#comment #this#is#a#comment

# Comments must be of utf-8 encoding
# Empty lines are ignored
# Otherwise, whitespace is *not* ignored, unless it precedes #
# Also remember that \r\n isn't a supported line ending.
```

```
# Config format:
# First, comes the basic config

# config version for backwards compatibility.
# must be the first meaningful line of any file.
config-version=0
# follow mounts. Options: false, true. Default is false.
# Enabling this can cause recursion!
follow-mounts=false
# Default is current working directory.
base-path=/home/user

# any unsupported config option results in an error

# Next comes the list of tags and rules
# A name in square bracket signifies the tag name
# Tags must be valid unicode
[sway]
# Next follows the list of rules.
# There are two basic kinds of rules - file and directory.
# Unlike comments, all paths must be in the filesystem-native encoding.
# Directory rules end with / and match the directory and its contents
.config/waybar/
# File rules don't end with / and strictly match a file
# Note that it doesn't match symlinks, only actual files!
.config/sway/config

# You can extend existing rules like this:
[extended-sway]
# @ followed by a tag name means include all the rules from said tag
@sway
.config/swaylock/

[all-symlinks-and-mount-points-and-symlink-dirs]
# "symlink" is a special tag that matches symlinks in path A that match
# path B (A is after the first semicolon, B is after the second)
@symlink;;/nix/store
# "mount-point" is a special tag that matches all mount points in a dir
# (not implemented right now)
@mount-point;
# "symlink-dir" is a special tag that matches all dirs that only contain
# symlinks to this path and other symlink dirs. It also matches empty
# dirs because I'm lazy.
@symlink-dir;;/nix/store
```

## Why are the rules so simple?

The goal is to be able to use it to define mountpoints for persistent
and temporary storage (the name is a reference to
[impermanence](https://github.com/nix-community/impermanence). This
means I have to keep the rules simple.

## CLI options

List all files matching the rules: `impertinence <subcommand> <config
file> <rule1> [<rule2> ...]

### Subcommands:

- `or` - list all files matching any of the rules
- `nor` - list all files matching none of the rules
- `and` - list all files matching at least two rules

## Potential future additions

- Rule inversion in config file (something like `!@symlink`,
  `!.config/swaylock/`)
- Escape sequences (`\x00`, `\0`, `\n`, ` \#`)
- Restrict tag names (forbid whitespace, etc)
- Ignore duplicate config options and redundant rules (optionally
  excluding tag inclusions from redundancy check)
- Multithreaded filesystem walking? (annoying and non-sequential)
- Better ways of matching symlinks/mountpoints (come on, why can't you
  only select symlinks in a certain dir?)

