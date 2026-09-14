//! The content tree the shell walks.
//!
//! Everything is in `content/` and baked in at compile time, so a new section
//! is a file and a line in [`ROOT`]. `ls`, `cat`, `cd` and completion all read
//! this tree, so they cannot disagree.

/// A named entry in a directory.
pub struct Entry {
    pub name: &'static str,
    pub node: Node,
}

/// A file the binary does not carry, because it changes while you are looking
/// at it. Reading one is a request, so whatever reads it answers later.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Live {
    /// The shared board. See `wall.rs`.
    Wall,
}

/// A directory, a file's text, a file the server holds, or something runnable.
pub enum Node {
    Dir(&'static [Entry]),
    File(&'static str),
    Live(Live),
    Program(&'static crate::program::Listing),
}

impl Node {
    /// One name in this node. Files have no children.
    #[must_use]
    pub fn child(&self, name: &str) -> Option<&'static Self> {
        match *self {
            Self::Dir(entries) => entries
                .iter()
                .find(|entry| entry.name == name)
                .map(|entry| &entry.node),
            Self::File(_) | Self::Live(_) | Self::Program(_) => None,
        }
    }

    /// The entries of a directory, or an empty slice for a file.
    #[must_use]
    pub const fn entries(&self) -> &'static [Entry] {
        match *self {
            Self::Dir(entries) => entries,
            Self::File(_) | Self::Live(_) | Self::Program(_) => &[],
        }
    }
}

/// The first paragraph of `about.txt`, used word for word by the banner and by
/// the meta description, so the intro is written once.
#[must_use]
pub fn intro() -> &'static str {
    const ABOUT: &str = include_str!("../content/documents/about.txt");
    ABOUT.split("\n\n").next().unwrap_or(ABOUT).trim_end()
}

pub const ROOT: Node = Node::Dir(&[
    // Programs.
    Entry {
        name: "bin",
        node: Node::Dir(crate::program::BIN),
    },
    // A home directory, laid out like one.
    Entry {
        name: "documents",
        node: Node::Dir(&[
            Entry {
                name: "about.txt",
                node: Node::File(include_str!("../content/documents/about.txt")),
            },
            Entry {
                name: "rust.txt",
                node: Node::File(include_str!("../content/documents/rust.txt")),
            },
            Entry {
                name: "linux.txt",
                node: Node::File(include_str!("../content/documents/linux.txt")),
            },
            Entry {
                name: "infra.txt",
                node: Node::File(include_str!("../content/documents/infra.txt")),
            },
            Entry {
                name: "design.txt",
                node: Node::File(include_str!("../content/documents/design.txt")),
            },
            Entry {
                name: "now.txt",
                node: Node::File(include_str!("../content/documents/now.txt")),
            },
            Entry {
                name: "contact.txt",
                node: Node::File(include_str!("../content/documents/contact.txt")),
            },
        ]),
    },
    Entry {
        name: "projects",
        node: Node::Dir(&[
            Entry {
                name: "audium.txt",
                node: Node::File(include_str!("../content/projects/audium.txt")),
            },
            Entry {
                name: "swagsh.txt",
                node: Node::File(include_str!("../content/projects/swagsh.txt")),
            },
            Entry {
                name: "carboxyl.txt",
                node: Node::File(include_str!("../content/projects/carboxyl.txt")),
            },
            Entry {
                name: "webpages.txt",
                node: Node::File(include_str!("../content/projects/webpages.txt")),
            },
        ]),
    },
    // The only file anyone else can write to, and the only one the server
    // holds rather than this binary.
    Entry {
        name: "var",
        node: Node::Dir(&[Entry {
            name: "wall.txt",
            node: Node::Live(Live::Wall),
        }]),
    },
]);

/// A path against a working directory. Handles `.`, `..` and `~`, and will not
/// walk above the top.
///
/// A leading `/` never resolves: this tree is a home directory and there is no
/// root above it.
///
/// Returns segments, which [`node_at`] turns back into a node.
#[must_use]
pub fn resolve(cwd: &[&'static str], path: &str) -> Option<Vec<&'static str>> {
    if path.starts_with('/') {
        return None;
    }

    let mut segments = if path.starts_with('~') {
        Vec::new()
    } else {
        cwd.to_vec()
    };

    for part in path.split('/') {
        match part {
            "" | "." | "~" => {}
            ".." => {
                segments.pop();
            }
            name => {
                // Only names that are really there can enter the path, which
                // is what keeps the segments `'static`.
                let here = node_at(&segments)?;
                let entry = here.entries().iter().find(|entry| entry.name == name)?;
                segments.push(entry.name);
            }
        }
    }
    Some(segments)
}

/// Segments back to the node they name.
#[must_use]
pub fn node_at(segments: &[&str]) -> Option<&'static Node> {
    let mut node = &ROOT;
    for segment in segments {
        node = node.child(segment)?;
    }
    Some(node)
}

/// A path the way the prompt and `pwd` show it, rooted at `~`.
#[must_use]
pub fn display_path(segments: &[&str]) -> String {
    if segments.is_empty() {
        "~".to_owned()
    } else {
        format!("~/{}", segments.join("/"))
    }
}
