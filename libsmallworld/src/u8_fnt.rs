//! Functions for reading and writing U8 archive filename tables (FNTs).

use std::collections::HashMap;
use std::fmt;
use std::io::{self, Read, Seek, SeekFrom, Write};

use binread::{BinReaderExt, NullString};
use itertools::Itertools;
use log::{debug, trace, warn};
use thiserror::Error;

use crate::util;

/// The "magic" identifier at the beginning of every U8 file. Often
/// written as `b"U\xaa8-"` (the ASCII characters of which lend "U8"
/// files their unofficial name).
pub const U8_MAGIC: u32 = 0x55aa382d;

/// All errors that can be encountered when parsing a U8 file.
#[non_exhaustive]
#[derive(Error, Debug)]
pub enum ParseU8Error {
    /// The file magic was incorrect.
    #[error("file is not a U8 archive (magic: {0:#x})")]
    BadMagic(u32),

    /// A node type other than 0 or 1 was found.
    #[error("unexpected node type: {0:?}")]
    UnexpectedNodeType(u8),

    /// All other errors.
    #[error("I/O error")]
    IoError(#[from] io::Error),
}

impl From<binread::Error> for ParseU8Error {
    fn from(error: binread::Error) -> ParseU8Error {
        ParseU8Error::IoError(io::Error::new(io::ErrorKind::Other, error))
    }
}

/// A struct representing a file node in a U8 filename table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct U8FileNode {
    /// The offset to the start of the file data. For convenience when
    /// writing U8 file data, this is always relative to the start of
    /// the FAT (unlike in the raw file data, where it's relative to the
    /// start of the file)
    pub offset: u32,

    /// The length of the file data.
    pub size: u32,
}

/// A type representing a folder node in a U8 filename table.
pub type U8FolderNode = HashMap<String, U8Node>;

/// An enum representing a U8 filename-table node. It can be either a
/// file (with offset and size), or a folder containing more `U8Node`s
/// indexed by name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum U8Node {
    File(U8FileNode),
    Folder(U8FolderNode),
}

impl U8Node {
    /// U8 node-type value that indicates that a node is a file.
    pub const FILE_TYPE: u8 = 0;
    /// U8 node-type value that indicates that a node is a folder.
    pub const FOLDER_TYPE: u8 = 1;

    /// Get the correct node-type value for this node.
    pub fn type_value(&self) -> u8 {
        match self {
            Self::File(_) => Self::FILE_TYPE,
            Self::Folder(_) => Self::FOLDER_TYPE,
        }
    }

    const DISPLAY_INDENT: usize = 2;
    const DISPLAY_OFFSET_RIGHT_EDGE: usize = 60;
    const DISPLAY_SIZE_RIGHT_EDGE: usize = 70;

    /// Format the `U8Node` as a string, with some amount of indentation.
    fn fmt_with_indent(
        &self,
        f: &mut fmt::Formatter<'_>,
        name: &str,
        indent: usize,
    ) -> fmt::Result {
        match self {
            Self::File(U8FileNode { offset, size }) => {
                write!(f, "{}{}", " ".repeat(indent), name)?;
                let mut amount_written = indent + name.len();
                // extra spaces in these format strings to ensure
                // there'll always be at least one before each number
                util::write_right_aligned_str(
                    f,
                    &mut amount_written,
                    Self::DISPLAY_OFFSET_RIGHT_EDGE,
                    &format!(" {offset:#x}"),
                )?;
                util::write_right_aligned_str(
                    f,
                    &mut amount_written,
                    Self::DISPLAY_SIZE_RIGHT_EDGE,
                    &format!(" {size:#x}"),
                )?;
            }
            Self::Folder(_) => {
                write!(f, "{}{}/", " ".repeat(indent), name)?;
                for (child_name, child) in self.iter() {
                    writeln!(f)?;
                    child.fmt_with_indent(f, child_name, indent + Self::DISPLAY_INDENT)?;
                }
            }
        };
        Ok(())
    }

    // Time to have two copies of every member-access / path traversal
    // function, which can't share any code despite being nearly
    // identical!! Rust sure is great, with no oversights at all.

    /// If this is a `U8Node::File`, return the associated `U8FileNode`.
    /// Else, return None.
    #[allow(dead_code)]
    pub fn as_file(&self) -> Option<&U8FileNode> {
        match self {
            Self::File(file) => Some(file),
            Self::Folder(_) => None,
        }
    }

    /// Mutable version of `.as_file()`.
    #[allow(dead_code)]
    pub fn as_mut_file(&mut self) -> Option<&mut U8FileNode> {
        match self {
            Self::File(file) => Some(file),
            Self::Folder(_) => None,
        }
    }

    /// If this is a `U8Node::Folder`, return the associated
    /// `U8FolderNode`. Else, return None.
    #[allow(dead_code)]
    pub fn as_folder(&self) -> Option<&U8FolderNode> {
        match self {
            Self::File(_) => None,
            Self::Folder(folder) => Some(folder),
        }
    }

    /// Mutable version of `.as_folder()`.
    #[allow(dead_code)]
    pub fn as_mut_folder(&mut self) -> Option<&mut U8FolderNode> {
        match self {
            Self::File(_) => None,
            Self::Folder(folder) => Some(folder),
        }
    }

    /// Iterate over a `U8Node::Folder`'s immediate descendants in the
    /// correct (i.e. case-insensitive alphabetical) order.
    /// For a `File`, just create an empty iterator.
    #[allow(dead_code)]
    pub fn iter(&self) -> std::vec::IntoIter<(&String, &Self)> {
        match self {
            U8Node::File(_) => Vec::new().into_iter(),
            U8Node::Folder(children) => children.iter().sorted_by_key(|x| x.0.to_lowercase()),
        }
    }

    /// Mutable version of `.iter()`.
    #[allow(dead_code)]
    pub fn iter_mut(&mut self) -> std::vec::IntoIter<(&String, &mut Self)> {
        match self {
            U8Node::File(_) => Vec::new().into_iter(),
            U8Node::Folder(children) => children.iter_mut().sorted_by_key(|x| x.0.to_lowercase()),
        }
    }

    /// Get an immediate child of a `U8Node::Folder` with the name
    /// provided (case-insensitively, the same way Nintendo does it).
    #[allow(dead_code)]
    pub fn child(&self, name: &str) -> Option<&Self> {
        if let U8Node::Folder(_) = self {
            let name = name.to_lowercase();
            for (child_name, child) in self.iter() {
                if child_name.to_lowercase() == name {
                    return Some(child);
                }
            }
        }
        None
    }

    /// Mutable version of `.child()`.
    #[allow(dead_code)]
    pub fn child_mut(&mut self, name: &str) -> Option<&mut Self> {
        if let U8Node::Folder(_) = self {
            let name = name.to_lowercase();
            for (child_name, child) in self.iter_mut() {
                if child_name.to_lowercase() == name {
                    return Some(child);
                }
            }
        }
        None
    }

    /// Get a descendant of a `U8Node::Folder`, following a path string
    /// separated by forward-slashes.
    #[allow(dead_code)]
    pub fn get(&self, path: &str) -> Option<&Self> {
        let mut current = self;

        for component in path.split('/') {
            if component.is_empty() {
                continue;
            }
            current = match current.child(component) {
                Some(child) => child,
                None => return None,
            }
        }
        Some(current)
    }

    /// Mutable version of `.get()`.
    pub fn get_mut(&mut self, path: &str) -> Option<&mut Self> {
        let mut current = self;

        for component in path.split('/') {
            if component.is_empty() {
                continue;
            }
            current = match current.child_mut(component) {
                Some(child) => child,
                None => return None,
            }
        }
        Some(current)
    }
}

impl fmt::Display for U8Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FILENAME")?;
        let mut amount_written = "FILENAME".len();
        util::write_right_aligned_str(
            f,
            &mut amount_written,
            Self::DISPLAY_OFFSET_RIGHT_EDGE,
            "OFFSET",
        )?;
        util::write_right_aligned_str(
            f,
            &mut amount_written,
            Self::DISPLAY_SIZE_RIGHT_EDGE + 1,
            "SIZE\n",
        )?;
        self.fmt_with_indent(f, "", 0)
    }
}

/// Read a U8 file's FNT.
///
/// Returns the new root node, and the offset to the start of the data
/// table (which all of the "offset" values in the FNT will be relative
/// to).
///
/// Assuming an `Ok` return value, the file will be left seeked to the
/// start of the FAT.
pub fn read<SR: Seek + Read>(file: &mut SR) -> Result<(U8Node, u32), ParseU8Error> {
    debug!("Reading U8 FNT");

    // Check magic, just to be sure the file looks sane
    file.seek(SeekFrom::Start(0))?;
    let magic: u32 = file.read_be()?;
    if magic != U8_MAGIC {
        return Err(ParseU8Error::BadMagic(magic));
    }

    // Read other header stuff
    file.seek(SeekFrom::Start(4))?;
    let root_node_offs: u32 = file.read_be()?;
    if root_node_offs != 0x20 {
        warn!("Unusual root node offset: {root_node_offs:#x}");
    }
    trace!("root_node_offs={root_node_offs:#x}");
    file.seek(SeekFrom::Current(4))?;
    let data_table_offs: u32 = file.read_be()?;
    trace!("data_table_offs={data_table_offs:#x}");

    // "Size" field of the root node tells us the total number of nodes;
    // this is how we calculate the offset of the string table
    file.seek(SeekFrom::Start((root_node_offs + 8).into()))?;
    let root_node_size: u32 = file.read_be()?;
    trace!("root_node_size={root_node_size:#x}");
    let string_table_offs = root_node_offs + 12 * root_node_size;
    trace!("string_table_offs={string_table_offs:#x}");

    // Inner function for recursion
    fn visit_node<SR: Seek + Read>(
        idx: &mut u32,
        file: &mut SR,
        root_node_offs: u32,
        string_table_offs: u32,
        data_table_offs: u32,
    ) -> Result<(String, U8Node), ParseU8Error> {
        let my_node_idx = *idx;
        let node_offs = root_node_offs + 12 * my_node_idx;
        trace!("Visiting node {idx} at {node_offs:#x}");

        // Read node header stuff
        file.seek(SeekFrom::Start(node_offs.into()))?;
        let first_u32: u32 = file.read_be()?;
        let node_type: u8 = (first_u32 >> 24).try_into().unwrap();
        let name_offs = first_u32 & 0x00ffffff;
        let data_offs: u32 = file.read_be()?;
        let size: u32 = file.read_be()?;
        trace!("Node {idx} header: ({node_type}, {name_offs:#x}, {data_offs:#x}, {size:#x})");

        // Read node name string
        file.seek(SeekFrom::Start((string_table_offs + name_offs).into()))?;
        let name = file.read_be::<NullString>()?.to_string();
        trace!("Node {idx} name: {name:?}");

        match node_type {
            U8Node::FILE_TYPE => {
                *idx += 1;
                Ok((
                    name,
                    U8Node::File(U8FileNode {
                        offset: data_offs - data_table_offs,
                        size,
                    }),
                ))
            }
            U8Node::FOLDER_TYPE => {
                let mut folder: U8FolderNode = U8FolderNode::new();

                trace!("Visiting children of node {my_node_idx}");
                *idx += 1;
                while *idx < size {
                    match visit_node(
                        idx,
                        file,
                        root_node_offs,
                        string_table_offs,
                        data_table_offs,
                    ) {
                        Ok(res_tuple) => folder.insert(res_tuple.0, res_tuple.1),
                        Err(why) => return Err(why),
                    };
                }
                trace!("Returning to parent dir (node {my_node_idx})");
                Ok((name, U8Node::Folder(folder)))
            }
            _ => Err(ParseU8Error::UnexpectedNodeType(node_type)),
        }
    }

    // Read starting at the root node, of course
    let res = (
        visit_node(
            &mut 0,
            file,
            root_node_offs,
            string_table_offs,
            data_table_offs,
        )?
        .1,
        data_table_offs,
    );

    // Leave the file seeked to the start of the FAT. Just makes
    // sense, since we've essentially read all of the file data up until
    // that point
    file.seek(SeekFrom::Start(data_table_offs.into()))?;

    trace!("Done reading U8 FNT");
    Ok(res)
}

/// Write a FNT to a U8 file.
///
/// Assuming an `Ok` return value, the file will be left seeked to the
/// start of the FAT.
pub fn write<SW: Seek + Write>(file: &mut SW, root: &U8Node) -> Result<(), io::Error> {
    debug!("Writing U8 FNT");

    let initial_file_offset = file.stream_position()?;
    trace!("initial_file_offset={initial_file_offset:#x}");

    // Write the constant parts of the header, leaving values we don't
    // yet know empty
    file.write_all(b"U\xaa8-\0\0\0\x20\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0")?;

    // We're not sure yet where the strings table will go (depends on
    // the length of the nodes table), so build it separately for now.
    let mut strings_table: Vec<u8> = Vec::new();

    // We won't know the FAT offset until the end, but we need to add
    // that to all of the nodes' file-data offset values. So instead of
    // writing those eagerly, we add them as (offset, value) pairs to
    // this list, and go back and write them at the very end, with the
    // FAT offset properly factored in.
    let mut file_data_offsets_to_write: Vec<(u64, u32)> = Vec::new();

    // Inner function for recursion
    fn visit_node<SW: Seek + Write>(
        node: &U8Node,
        name: &str,
        idx: &mut u32,
        recursion_depth: i32,
        file: &mut SW,
        strings_table: &mut Vec<u8>,
        file_data_offsets_to_write: &mut Vec<(u64, u32)>,
    ) -> Result<(), io::Error> {
        trace!("Visiting node {idx}: {name:?}");

        // Get an index, and determine our offset in the nodes table
        let my_node_idx = *idx;
        *idx += 1;
        let my_node_offs = file.stream_position()?;
        trace!("node_offs={my_node_offs:#x}");

        // Add the name
        let name_offs: u32 = strings_table.len().try_into().unwrap();
        trace!("name_offs={name_offs:#x}");
        strings_table.extend_from_slice(name.as_bytes());
        strings_table.push(0);

        // Add placeholder data for this node, since we need to reserve
        // space but don't yet know what values to write
        file.write_all(&[0; 12])?;

        // Figure out what values to write!
        let (data_offs, size) = match node {
            U8Node::File(U8FileNode { offset, size }) => {
                // We don't know the FAT offset yet, so we can't
                // determine our own data offset. So add an entry to the
                // list, and use 0 as a placeholder for now.
                trace!(
                    "Adding to the fix-ups list: ({:#x}, {:#x})",
                    my_node_offs + 4,
                    *offset
                );
                file_data_offsets_to_write.push((my_node_offs + 4, *offset));
                (0_u32, *size)
            }
            U8Node::Folder(_) => {
                trace!("Visiting children of node {my_node_idx}");
                for (child_name, child) in node.iter() {
                    visit_node(
                        child,
                        child_name,
                        idx,
                        recursion_depth + 1,
                        file,
                        strings_table,
                        file_data_offsets_to_write,
                    )?;
                }
                trace!("Returning to parent dir (node {my_node_idx})");
                // below: casting from i32 to u32 and saturating negative values to 0
                (recursion_depth.try_into().unwrap_or(0), *idx)
            }
        };

        let saved_offs = file.stream_position()?;
        trace!("saved_offs={saved_offs:#x}");

        // Go back and fill in the FNT values, now that we know them
        // (except for file nodes' data offsets, which will be filled in
        // for real later)
        trace!(
            "Writing new node header: ({}, {name_offs:#x}, {data_offs:#x}, {size:#x})",
            node.type_value()
        );
        file.seek(SeekFrom::Start(my_node_offs))?;
        let first_u32: u32 = (u32::from(node.type_value()) << 24) | name_offs;
        file.write_all(&first_u32.to_be_bytes())?;
        file.write_all(&data_offs.to_be_bytes())?;
        file.write_all(&size.to_be_bytes())?;

        // (Go back to where we were)
        file.seek(SeekFrom::Start(saved_offs))?;

        Ok(())
    }

    // Do all the things!
    visit_node(
        root,
        "",
        &mut 0,
        -1,
        file,
        &mut strings_table,
        &mut file_data_offsets_to_write,
    )?;

    // Append the strings table, and make a note of the current length
    // relative to 0x20 (this is a value we'll have to write to the
    // header)
    file.write_all(&strings_table)?;
    let end_of_header: u32 = (file.stream_position()? - 0x20 - initial_file_offset)
        .try_into()
        .unwrap();
    trace!("end_of_header={end_of_header:#x}");

    // Align to 0x20 and make another note of the current length
    util::write_zeros_to_align_to(file, 0x20, initial_file_offset)?;
    let data_table_offset: u32 = (file.stream_position()? - initial_file_offset)
        .try_into()
        .unwrap();
    trace!("data_table_offset={data_table_offset:#x}");

    // Write the real data offsets for all of the file nodes, now that
    // we can
    trace!(
        "Fixing {} file-data offsets",
        file_data_offsets_to_write.len()
    );
    for (offs, relative_value) in file_data_offsets_to_write {
        file.seek(SeekFrom::Start(offs as u64))?;
        file.write_all(&(data_table_offset + relative_value).to_be_bytes())?;
    }

    // Add the final header values and return
    trace!("Writing the remaining header values");
    file.seek(SeekFrom::Start(initial_file_offset + 8))?;
    file.write_all(&end_of_header.to_be_bytes())?;
    file.write_all(&data_table_offset.to_be_bytes())?;

    file.seek(SeekFrom::Start(
        initial_file_offset + u64::from(data_table_offset),
    ))?;
    trace!("Done writing U8 FNT");
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unnecessary_wraps)]
mod tests {
    use super::*;
    use std::io::Cursor;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    mod u8node_display {
        use super::*;

        #[test]
        fn test_empty_fnt() -> TestResult {
            let root = U8Node::Folder(U8FolderNode::new());
            assert_eq!(
                format!("\n{}", root),
                r"
FILENAME                                              OFFSET      SIZE
/"
            );
            Ok(())
        }

        #[test]
        fn test_simple_fnt() -> TestResult {
            let root = U8Node::Folder(U8FolderNode::from([
                (
                    "a".to_owned(),
                    U8Node::File(U8FileNode {
                        offset: 0x1,
                        size: 0x3,
                    }),
                ),
                (
                    "bb".to_owned(),
                    U8Node::Folder(U8FolderNode::from([
                        (
                            "ccc".to_owned(),
                            U8Node::File(U8FileNode {
                                offset: 0x5,
                                size: 0x7,
                            }),
                        ),
                        (
                            "dddd".to_owned(),
                            U8Node::File(U8FileNode {
                                offset: 0x9,
                                size: 0xb,
                            }),
                        ),
                    ])),
                ),
                (
                    "eeeee".to_owned(),
                    U8Node::File(U8FileNode {
                        offset: 0xd,
                        size: 0xf,
                    }),
                ),
            ]));

            assert_eq!(
                format!("\n{}", root),
                r"
FILENAME                                              OFFSET      SIZE
/
  a                                                      0x1       0x3
  bb/
    ccc                                                  0x5       0x7
    dddd                                                 0x9       0xb
  eeeee                                                  0xd       0xf"
            );
            Ok(())
        }
    }

    mod read {
        use super::*;

        #[test]
        fn test_empty_fnt() -> TestResult {
            let mut cursor = Cursor::new(
                concat_bytes!(
                    b"U\xaa8-\0\0\0 \0\0\0\r\0\0\0@\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
                    b"\x01\0\0\0\0\0\0\0\0\0\0\x01\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
                )
                .to_vec(),
            );
            let (root, data_table_offs) = read(&mut cursor)?;
            assert_eq!(root, U8Node::Folder(U8FolderNode::new()));
            assert_eq!(data_table_offs, 0x40);
            Ok(())
        }

        #[test]
        fn test_simple_fnt() -> TestResult {
            let mut cursor = Cursor::new(
                concat_bytes!(
                b"U\xaa8-\0\0\0 \0\0\0]\0\0\0\x80\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
                b"\x01\0\0\0\0\0\0\0\0\0\0\x06\0\0\0\x01\0\0\0\x81\0\0\0\x03\x01\0\0\x03\0\0\0\0",
                b"\0\0\0\x05\0\0\0\x06\0\0\0\x85\0\0\0\x07\0\0\0\n\0\0\0\x89\0\0\0\x0b\0\0\0\x0f",
                b"\0\0\0\x8d\0\0\0\x0f\0a\0bb\0ccc\0dddd\0eeeee\0\0\0\0",
            )
                .to_vec(),
            );
            let (root, data_table_offs) = read(&mut cursor)?;
            assert_eq!(
                root,
                U8Node::Folder(U8FolderNode::from([
                    (
                        "a".to_owned(),
                        U8Node::File(U8FileNode { offset: 1, size: 3 })
                    ),
                    (
                        "bb".to_owned(),
                        U8Node::Folder(U8FolderNode::from([
                            (
                                "ccc".to_owned(),
                                U8Node::File(U8FileNode { offset: 5, size: 7 })
                            ),
                            (
                                "dddd".to_owned(),
                                U8Node::File(U8FileNode {
                                    offset: 9,
                                    size: 11
                                })
                            ),
                        ]))
                    ),
                    (
                        "eeeee".to_owned(),
                        U8Node::File(U8FileNode {
                            offset: 13,
                            size: 15
                        })
                    ),
                ]))
            );
            assert_eq!(data_table_offs, 0x80);
            Ok(())
        }
    }

    mod write {
        use super::*;

        /// Helper function to automatically test a bunch of variations
        /// on a FNT-writing scenario
        fn test_writing_a_fnt(root: &U8Node, expected_output: &[u8]) -> TestResult {
            // Test files with various amounts of data already written to them...
            for initial_offset in [0, 1, 2, 3, 4, 8, 12, 50] {
                // ...and with various amounts of data already written past the cursor position
                for already_written_past_there in [0, 1, 2, 3, 30, 300, 3000] {
                    let total_length = initial_offset + already_written_past_there;

                    // Create a buffer for the file data (initialize it to
                    // something non-null, in case the function under test
                    // forgets to fill in null bytes where needed), and seek to
                    // the starting offset
                    let mut cursor = Cursor::new(vec![7; total_length]);
                    cursor.seek(SeekFrom::Start(initial_offset.try_into()?))?;

                    // Test the function
                    write(&mut cursor, root)?;

                    // Check that it wrote the expected output
                    let end_of_fnt = cursor.stream_position()?.try_into()?;
                    assert_eq!(
                        &cursor.into_inner()[initial_offset..end_of_fnt],
                        expected_output
                    );
                }
            }
            Ok(())
        }

        #[test]
        fn test_empty_fnt() -> TestResult {
            test_writing_a_fnt(
                &U8Node::Folder(U8FolderNode::new()),
                concat_bytes!(
                    b"U\xaa8-\0\0\0 \0\0\0\r\0\0\0@\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
                    b"\x01\0\0\0\0\0\0\0\0\0\0\x01\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
                ),
            )
        }

        #[test]
        fn test_simple_fnt() -> TestResult {
            test_writing_a_fnt(
                &U8Node::Folder(U8FolderNode::from([
                    ("a".to_owned(), U8Node::File(U8FileNode{offset: 1, size: 3})),
                    ("bb".to_owned(), U8Node::Folder(U8FolderNode::from([
                        ("ccc".to_owned(), U8Node::File(U8FileNode{offset: 5, size: 7})),
                        ("dddd".to_owned(), U8Node::File(U8FileNode{offset: 9, size: 11})),
                    ]))),
                    ("eeeee".to_owned(), U8Node::File(U8FileNode{offset: 13, size: 15})),
                ])),
                concat_bytes!(
                    b"U\xaa8-\0\0\0 \0\0\0]\0\0\0\x80\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
                    b"\x01\0\0\0\0\0\0\0\0\0\0\x06\0\0\0\x01\0\0\0\x81\0\0\0\x03\x01\0\0\x03\0\0\0\0",
                    b"\0\0\0\x05\0\0\0\x06\0\0\0\x85\0\0\0\x07\0\0\0\n\0\0\0\x89\0\0\0\x0b\0\0\0\x0f",
                    b"\0\0\0\x8d\0\0\0\x0f\0a\0bb\0ccc\0dddd\0eeeee\0\0\0\0",
                ),
            )
        }
    }
}
