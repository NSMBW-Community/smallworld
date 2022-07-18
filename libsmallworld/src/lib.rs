//! The library that powers smallworld. Handles everything other than
//! reading CLI arguments.

#![feature(concat_bytes)]
#![feature(int_roundings)]

mod openingtitle_filename_constants;
mod u8_fnt;
mod util;

use std::collections::HashMap;
use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};
use std::str::FromStr;

use enumflags2::{bitflags, BitFlags};
use log::{debug, info, trace};
use thiserror::Error;

use crate::openingtitle_filename_constants::ALL_FILENAMES;
use crate::u8_fnt::{U8FileNode, U8FolderNode, U8Node};

pub use crate::u8_fnt::ParseU8Error;

/// The path to openingTitle.arc's "anim" folder.
const ANIM_FOLDER_PATH: &str = "/arc/anim";

/// The path to openingTitle.arc's "blyt" folder.
const BLYT_FOLDER_PATH: &str = "/arc/blyt";

/// All unique regions of *New Super Mario Bros. Wii*, using the letter
/// names from their game codes (SMN**P**01, SMN**E**01, etc.).
#[bitflags]
#[repr(u32)]
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum Region {
    /// International
    P,
    /// North America
    E,
    /// Japan
    J,
    /// Korea
    K,
    /// Taiwan
    W,
    /// China
    C,
}

/// A `BitFlags` of `Region`s, useful for selecting regions that an
/// openingTitle.arc should include filenames for.
pub type RegionBitFlags = BitFlags<Region, u32>;

// This is dumb, but apparently required for
// `RegionBitFlags::from_iter(&[Region])` to compile
impl From<&Region> for RegionBitFlags {
    fn from(region: &Region) -> RegionBitFlags {
        RegionBitFlags::from(*region)
    }
}

impl Region {
    /// A list of all regions, in their "default" order. If conflicting
    /// copies of a region-specific file are found (and the
    /// configuration options request that files be overwritten instead
    /// of returning `Err`), smallworld will by default use the one
    /// corresponding to the region listed first here.
    ///
    /// This is public because the default order in which files are
    /// prioritized is a documented guarantee. The array may change in
    /// the future, though, if new versions of NSMBW are ever released.
    pub const DEFAULT_ORDER: [Self; 6] = [Self::P, Self::E, Self::J, Self::K, Self::W, Self::C];
}

/// An error that can occur when parsing a `Region` from a string.
#[non_exhaustive]
#[derive(Error, Debug)]
pub enum ParseRegionError {
    /// The string couldn't be recognized as a region name.
    #[error("unknown region name {0:?}")]
    UnknownRegionName(String),
}

impl FromStr for Region {
    type Err = ParseRegionError;

    fn from_str(s: &str) -> Result<Self, ParseRegionError> {
        match &s.to_uppercase() as &str {
            "P" => Ok(Self::P),
            "E" => Ok(Self::E),
            "J" => Ok(Self::J),
            "K" => Ok(Self::K),
            "W" => Ok(Self::W),
            "C" => Ok(Self::C),
            _ => Err(ParseRegionError::UnknownRegionName(s.to_owned())),
        }
    }
}

impl From<Region> for &str {
    fn from(value: Region) -> Self {
        match value {
            Region::P => "P",
            Region::E => "E",
            Region::J => "J",
            Region::K => "K",
            Region::W => "W",
            Region::C => "C",
        }
    }
}

impl From<&Region> for &str {
    fn from(value: &Region) -> Self {
        Self::from(*value)
    }
}

/// Indicates how a function should proceed if it finds that it needs to
/// merge two or more conflicting things.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Default)]
pub enum ConflictStrategy {
    /// Treat the conflict as an error and return `Err`. For safety,
    /// this is the default.
    #[default]
    Fail,
    /// Select one of the conflicting things, discard the other one(s),
    /// and continue.
    Overwrite,
}

/// All errors that can be encountered when converting an
/// openingTitle.arc between regions.
#[non_exhaustive]
#[derive(Error, Debug)]
pub enum ConvertOpeningTitleBetweenRegionsError {
    /// The openingTitle.arc is an invalid U8 archive file.
    #[error("invalid U8 file")]
    InvalidU8File(#[from] u8_fnt::ParseU8Error),

    /// The openingTitle.arc is wrong in a structural sense (not a
    /// parsing sense).
    #[error("{0}")]
    InvalidOpeningTitleStructure(String),

    /// At least one required file was missing. The file might exist but
    /// for a different region, or it might just be completely absent.
    #[error("{0} not found")]
    MissingFiles(String),

    /// Two files that need to be merged have different data.
    #[error("conflicting files: {0:?} and {1:?} are different")]
    FileDataConflict(String, String),

    /// A filename that needs to be added already exists.
    #[error("{0:?} already exists")]
    FilenameAlreadyExists(String),

    /// All other errors.
    #[error("I/O error")]
    IoError(#[from] io::Error),
}

/// A `U8FileNode` with a filename attached to it. The filename is an
/// owned string, honestly to make lifetimes simpler.
#[derive(Clone, Debug, PartialEq, Eq)]
struct NamedU8FileNode {
    node: U8FileNode,
    filename: String,
}

/// Contains `Option`al `NamedU8FileNode`s that correspond to the five
/// files with region-dependent filenames ("regional files"). Useful
/// while searching for files in the input arc and resolving conflicts
/// / missing files / etc.
#[derive(Debug, PartialEq, Eq)]
struct OptionalNamedRegionalFiles {
    in_press_brlan: Option<NamedU8FileNode>,
    in_title_brlan: Option<NamedU8FileNode>,
    loop_press_brlan: Option<NamedU8FileNode>,
    out_press_brlan: Option<NamedU8FileNode>,
    brlyt: Option<NamedU8FileNode>,
}

/// Contains `U8FileNode`s that correspond to the five files with
/// region-dependent filenames ("regional files"). Useful for when the
/// exact set of files to use has been chosen.
#[derive(Debug, PartialEq, Eq)]
struct RegionalFiles {
    in_press_brlan: U8FileNode,
    in_title_brlan: U8FileNode,
    loop_press_brlan: U8FileNode,
    out_press_brlan: U8FileNode,
    brlyt: U8FileNode,
}

/// Get a reference to the /arc/anim `U8FolderNode`.
#[allow(dead_code)] // not actually dead code, but Clippy thinks it is?
fn get_anim_folder(fnt: &U8Node) -> Result<&U8FolderNode, ConvertOpeningTitleBetweenRegionsError> {
    fnt.get(ANIM_FOLDER_PATH)
        .ok_or_else(|| {
            ConvertOpeningTitleBetweenRegionsError::InvalidOpeningTitleStructure(format!(
                "{} folder not found",
                ANIM_FOLDER_PATH
            ))
        })?
        .as_folder()
        .ok_or_else(|| {
            ConvertOpeningTitleBetweenRegionsError::InvalidOpeningTitleStructure(format!(
                "{} wasn't a folder",
                ANIM_FOLDER_PATH
            ))
        })
}

/// Mutable version of `get_anim_folder`.
#[allow(dead_code)]
fn get_mut_anim_folder(
    fnt: &mut U8Node,
) -> Result<&mut U8FolderNode, ConvertOpeningTitleBetweenRegionsError> {
    fnt.get_mut(ANIM_FOLDER_PATH)
        .ok_or_else(|| {
            ConvertOpeningTitleBetweenRegionsError::InvalidOpeningTitleStructure(format!(
                "{} folder not found",
                ANIM_FOLDER_PATH
            ))
        })?
        .as_mut_folder()
        .ok_or_else(|| {
            ConvertOpeningTitleBetweenRegionsError::InvalidOpeningTitleStructure(format!(
                "{} wasn't a folder",
                ANIM_FOLDER_PATH
            ))
        })
}

/// Get a reference to the /arc/blyt `U8FolderNode`.
#[allow(dead_code)] // not actually dead code, but Clippy thinks it is?
fn get_blyt_folder(fnt: &U8Node) -> Result<&U8FolderNode, ConvertOpeningTitleBetweenRegionsError> {
    fnt.get(BLYT_FOLDER_PATH)
        .ok_or_else(|| {
            ConvertOpeningTitleBetweenRegionsError::InvalidOpeningTitleStructure(format!(
                "{} folder not found",
                BLYT_FOLDER_PATH
            ))
        })?
        .as_folder()
        .ok_or_else(|| {
            ConvertOpeningTitleBetweenRegionsError::InvalidOpeningTitleStructure(format!(
                "{} wasn't a folder",
                BLYT_FOLDER_PATH
            ))
        })
}

/// Mutable version of `get_blyt_folder`.
#[allow(dead_code)]
fn get_mut_blyt_folder(
    fnt: &mut U8Node,
) -> Result<&mut U8FolderNode, ConvertOpeningTitleBetweenRegionsError> {
    fnt.get_mut(BLYT_FOLDER_PATH)
        .ok_or_else(|| {
            ConvertOpeningTitleBetweenRegionsError::InvalidOpeningTitleStructure(format!(
                "{} folder not found",
                BLYT_FOLDER_PATH
            ))
        })?
        .as_mut_folder()
        .ok_or_else(|| {
            ConvertOpeningTitleBetweenRegionsError::InvalidOpeningTitleStructure(format!(
                "{} wasn't a folder",
                BLYT_FOLDER_PATH
            ))
        })
}

/// Remove all files corresponding to the requested regions from a FNT
/// root node, and create a `HashMap` mapping each region to the file
/// nodes from it that were found and removed.
fn remove_regional_files(
    fnt: &mut U8Node,
    regions: RegionBitFlags,
) -> Result<HashMap<Region, OptionalNamedRegionalFiles>, ConvertOpeningTitleBetweenRegionsError> {
    let mut map = HashMap::new();

    // Prepare all the structs in the map
    for region in regions {
        map.insert(
            region,
            OptionalNamedRegionalFiles {
                in_press_brlan: None,
                in_title_brlan: None,
                loop_press_brlan: None,
                out_press_brlan: None,
                brlyt: None,
            },
        );
    }

    // Get the /arc/anim folder
    let folder = get_mut_anim_folder(fnt)?;

    // Remove all requested files from /arc/anim
    for region in regions {
        let region_name = region.into();
        let mut region_files = map.get_mut(&region).unwrap();

        let filename = ALL_FILENAMES[region_name].in_press_brlan;
        if let Some(U8Node::File(file_node)) = folder.remove(filename) {
            trace!("Removing {:?}", filename);
            region_files.in_press_brlan = Some(NamedU8FileNode {
                node: file_node,
                filename: filename.to_owned(),
            });
        }

        let filename = ALL_FILENAMES[region_name].in_title_brlan;
        if let Some(U8Node::File(file_node)) = folder.remove(filename) {
            trace!("Removing {:?}", filename);
            region_files.in_title_brlan = Some(NamedU8FileNode {
                node: file_node,
                filename: filename.to_owned(),
            });
        }

        let filename = ALL_FILENAMES[region_name].loop_press_brlan;
        if let Some(U8Node::File(file_node)) = folder.remove(filename) {
            trace!("Removing {:?}", filename);
            region_files.loop_press_brlan = Some(NamedU8FileNode {
                node: file_node,
                filename: filename.to_owned(),
            });
        }

        let filename = ALL_FILENAMES[region_name].out_press_brlan;
        if let Some(U8Node::File(file_node)) = folder.remove(filename) {
            trace!("Removing {:?}", filename);
            region_files.out_press_brlan = Some(NamedU8FileNode {
                node: file_node,
                filename: filename.to_owned(),
            });
        }
    }

    // This is the same as above, but for the BRLYT.
    // We do it separately because we can only have one mutable borrow
    // on `fnt` at a time (and thus can't have both the anim and blyt
    // folders referenced mutably simultaneously)

    let folder = get_mut_blyt_folder(fnt)?;

    for region in regions {
        let region_name = region.into();
        let mut region_files = map.get_mut(&region).unwrap();

        let filename = ALL_FILENAMES[region_name].brlyt;
        if let Some(U8Node::File(file_node)) = folder.remove(filename) {
            trace!("Removing {:?}", filename);
            region_files.brlyt = Some(NamedU8FileNode {
                node: file_node,
                filename: filename.to_owned(),
            });
        }
    }

    Ok(map)
}

/// Compare the data for two `NamedU8FileNode`s, and return `Err` if
/// they don't match.
///
/// If `previous_found_file` is None, it's set to a clone of the other
/// file, and no actual check (or hashing) is performed.
///
/// If `previous_found_file` is Some, the new file is hashed (and also
/// the old file, if it hasn't yet been hashed), and the hashes are
/// compared.
fn check_file_pair_for_conflicts<SR: Seek + Read>(
    previous_found_file: &mut Option<NamedU8FileNode>,
    previous_found_file_hash: &mut u64,
    found_file: &NamedU8FileNode,
    data_table_offs: u32,
    reader: &mut SR,
) -> Result<(), ConvertOpeningTitleBetweenRegionsError> {
    if let Some(previous_found_file) = &*previous_found_file {
        if found_file.node == previous_found_file.node {
            // The nodes are identical, so we know it's safe even without
            // checking the data.
            return Ok(());
        }

        // Ensure that the first file's hash has been calculated
        if *previous_found_file_hash == 0 {
            trace!(
                "Calculating hash of {:?} ({:#x}-{:#x})",
                previous_found_file.filename,
                previous_found_file.node.offset,
                previous_found_file.node.offset + previous_found_file.node.size
            );

            *previous_found_file_hash = util::calc_hash_from_file_slice(
                reader,
                (data_table_offs + previous_found_file.node.offset).into(),
                previous_found_file.node.size.try_into().unwrap(),
            )?;
        }

        // Calculate the hash of the new file
        trace!(
            "Calculating hash of {:?} ({:#x}-{:#x})",
            found_file.filename,
            found_file.node.offset,
            found_file.node.offset + found_file.node.size
        );

        let new_hash = util::calc_hash_from_file_slice(
            reader,
            (data_table_offs + found_file.node.offset).into(),
            found_file.node.size.try_into().unwrap(),
        )?;

        if *previous_found_file_hash != new_hash {
            return Err(ConvertOpeningTitleBetweenRegionsError::FileDataConflict(
                previous_found_file.filename.clone(),
                found_file.filename.clone(),
            ));
        }
    } else {
        // We'll calculate the hash later, only if we actually find
        // another file that it needs to be compared against
        *previous_found_file = Some(found_file.clone());
    }

    Ok(())
}

/// Check all of the provided regional files for data conflicts, and
/// return an `Err` if any are found.
fn check_all_files_for_conflicts<SR: Seek + Read>(
    all_regional_files: &HashMap<Region, OptionalNamedRegionalFiles>,
    data_table_offs: u32,
    mut reader: &mut SR,
) -> Result<(), ConvertOpeningTitleBetweenRegionsError> {
    // Keep track of one copy of each file, and their hashes
    let mut in_press_brlan = None;
    let mut in_title_brlan = None;
    let mut loop_press_brlan = None;
    let mut out_press_brlan = None;
    let mut brlyt = None;
    let mut in_press_brlan_hash = 0;
    let mut in_title_brlan_hash = 0;
    let mut loop_press_brlan_hash = 0;
    let mut out_press_brlan_hash = 0;
    let mut brlyt_hash = 0;

    for regional_files in all_regional_files.values() {
        if let Some(file) = &regional_files.in_press_brlan {
            check_file_pair_for_conflicts(
                &mut in_press_brlan,
                &mut in_press_brlan_hash,
                file,
                data_table_offs,
                &mut reader,
            )?;
        }

        if let Some(file) = &regional_files.in_title_brlan {
            check_file_pair_for_conflicts(
                &mut in_title_brlan,
                &mut in_title_brlan_hash,
                file,
                data_table_offs,
                &mut reader,
            )?;
        }

        if let Some(file) = &regional_files.loop_press_brlan {
            check_file_pair_for_conflicts(
                &mut loop_press_brlan,
                &mut loop_press_brlan_hash,
                file,
                data_table_offs,
                &mut reader,
            )?;
        }

        if let Some(file) = &regional_files.out_press_brlan {
            check_file_pair_for_conflicts(
                &mut out_press_brlan,
                &mut out_press_brlan_hash,
                file,
                data_table_offs,
                &mut reader,
            )?;
        }

        if let Some(file) = &regional_files.brlyt {
            check_file_pair_for_conflicts(
                &mut brlyt,
                &mut brlyt_hash,
                file,
                data_table_offs,
                &mut reader,
            )?;
        }
    }

    Ok(())
}

/// Select exactly one of each regional file, favoring the ones from
/// regions that appear earliest in `from_regions`.
fn select_regional_files(
    all_regional_files: &HashMap<Region, OptionalNamedRegionalFiles>,
    from_regions: &[Region],
) -> Result<RegionalFiles, ConvertOpeningTitleBetweenRegionsError> {
    // We're going to search for these
    let mut in_press_brlan = None;
    let mut in_title_brlan = None;
    let mut loop_press_brlan = None;
    let mut out_press_brlan = None;
    let mut brlyt = None;

    for region in from_regions {
        if let Some(regional_files) = all_regional_files.get(region) {
            if let Some(file_node) = &regional_files.in_press_brlan {
                in_press_brlan = in_press_brlan.or_else(|| Some(file_node.node.clone()));
            }

            if let Some(file_node) = &regional_files.in_title_brlan {
                in_title_brlan = in_title_brlan.or_else(|| Some(file_node.node.clone()));
            }

            if let Some(file_node) = &regional_files.loop_press_brlan {
                loop_press_brlan = loop_press_brlan.or_else(|| Some(file_node.node.clone()));
            }

            if let Some(file_node) = &regional_files.out_press_brlan {
                out_press_brlan = out_press_brlan.or_else(|| Some(file_node.node.clone()));
            }

            if let Some(file_node) = &regional_files.brlyt {
                brlyt = brlyt.or_else(|| Some(file_node.node.clone()));
            }
        }
    }

    Ok(RegionalFiles {
        in_press_brlan: in_press_brlan.ok_or_else(|| {
            ConvertOpeningTitleBetweenRegionsError::MissingFiles("inPress brlan".to_owned())
        })?,
        in_title_brlan: in_title_brlan.ok_or_else(|| {
            ConvertOpeningTitleBetweenRegionsError::MissingFiles("inTitle brlan".to_owned())
        })?,
        loop_press_brlan: loop_press_brlan.ok_or_else(|| {
            ConvertOpeningTitleBetweenRegionsError::MissingFiles("loopPress brlan".to_owned())
        })?,
        out_press_brlan: out_press_brlan.ok_or_else(|| {
            ConvertOpeningTitleBetweenRegionsError::MissingFiles("outPress brlan".to_owned())
        })?,
        brlyt: brlyt.ok_or_else(|| {
            ConvertOpeningTitleBetweenRegionsError::MissingFiles("brlyt".to_owned())
        })?,
    })
}

/// Add new entries to the U8 FNT pointing to (clones of) the indicated
/// regional-file nodes, with filenames appropriate for the indicated
/// output region.
///
/// If `ignore_conflicts` is `true` and any of the filenames already
/// exist, they'll be overwritten. Otherwise, `Err` will be returned.
fn add_new_filenames(
    fnt: &mut U8Node,
    regional_files: &RegionalFiles,
    regions: RegionBitFlags,
    filename_conflict_strategy: ConflictStrategy,
) -> Result<(), ConvertOpeningTitleBetweenRegionsError> {
    /// Helper function to add a single file to a folder, and perform
    /// the optional conflict check if enabled
    fn insert(
        folder: &mut U8FolderNode,
        filename: &str,
        file_node: &U8FileNode,
        filename_conflict_strategy: ConflictStrategy,
    ) -> Result<(), ConvertOpeningTitleBetweenRegionsError> {
        if filename_conflict_strategy == ConflictStrategy::Fail && folder.contains_key(filename) {
            return Err(
                ConvertOpeningTitleBetweenRegionsError::FilenameAlreadyExists(filename.to_owned()),
            );
        }

        folder.insert(filename.to_owned(), U8Node::File(file_node.clone()));

        Ok(())
    }

    for this_region in regions {
        let this_region_name = this_region.into();
        trace!("Adding filenames for {this_region_name:?}");

        let folder = get_mut_anim_folder(fnt)?;

        insert(
            folder,
            ALL_FILENAMES[this_region_name].in_press_brlan,
            &regional_files.in_press_brlan,
            filename_conflict_strategy,
        )?;
        insert(
            folder,
            ALL_FILENAMES[this_region_name].in_title_brlan,
            &regional_files.in_title_brlan,
            filename_conflict_strategy,
        )?;
        insert(
            folder,
            ALL_FILENAMES[this_region_name].loop_press_brlan,
            &regional_files.loop_press_brlan,
            filename_conflict_strategy,
        )?;
        insert(
            folder,
            ALL_FILENAMES[this_region_name].out_press_brlan,
            &regional_files.out_press_brlan,
            filename_conflict_strategy,
        )?;

        let folder = get_mut_blyt_folder(fnt)?;

        insert(
            folder,
            ALL_FILENAMES[this_region_name].brlyt,
            &regional_files.brlyt,
            filename_conflict_strategy,
        )?;
    }

    Ok(())
}

/// Given a U8 root node and a reader for its corresponding file data,
/// build a new FAT and update the FNT offsets to match. The FAT will be
/// written starting at the writer's current position.
///
/// File data may be shuffled in order to match the order Nintendo would
/// usually put them in. More importantly for this application, files
/// with matching offsets are guaranteed to be maintained as such.
fn build_new_fat<SR: Seek + Read, SW: Seek + Write>(
    fnt: &mut U8Node,
    data_table_offs: u32,
    in_file: &mut SR,
    out_file: &mut SW,
) -> Result<(), ConvertOpeningTitleBetweenRegionsError> {
    let initial_fat_offset = out_file.stream_position()?;

    // A mapping {old_offset: new_offset}, which lets us keep track of
    // where we've remapped the original file data offsets to.
    // This is needed for detecting and properly handling files that
    // have the same offsets.
    let mut offset_remapping = HashMap::new();

    // Inner function for recursion
    fn visit_node<SR: Seek + Read, SW: Seek + Write>(
        name: &str,
        node: &mut U8Node,
        data_table_offs: u32,
        in_file: &mut SR,
        out_file: &mut SW,
        initial_fat_offset: u64,
        offset_remapping: &mut HashMap<u32, u32>,
    ) -> Result<(), ConvertOpeningTitleBetweenRegionsError> {
        trace!("Visiting {name:?}");

        match node {
            U8Node::File(U8FileNode {
                ref mut offset,
                size,
            }) => {
                // Have we seen this offset before?
                *offset = if let Some(value) = offset_remapping.get(offset) {
                    // Yes? Re-use the same updated offset, and move on
                    *value
                } else {
                    // Align to 0x20,
                    util::write_zeros_to_align_to(out_file, 0x20, 0)?;
                    // check the new offset,
                    let new_file_pos: u32 = (out_file.stream_position()? - initial_fat_offset)
                        .try_into()
                        .unwrap();
                    trace!("Moved from {:#x} to {:#x}", *offset, new_file_pos);
                    // seek the input file to the original offset,
                    in_file.seek(SeekFrom::Start((data_table_offs + *offset).into()))?;
                    // copy the file data across,
                    util::read_from_into(in_file, out_file, (*size).try_into().unwrap())?;
                    // and add a new entry to offset_remapping.
                    offset_remapping.insert(*offset, new_file_pos);
                    // And update the actual node offset
                    new_file_pos
                }
            }
            U8Node::Folder(_) => {
                // Visit all the children recursively
                trace!("Visiting children of {name:?}");
                for (child_name, child) in node.iter_mut() {
                    visit_node(
                        child_name,
                        child,
                        data_table_offs,
                        in_file,
                        out_file,
                        initial_fat_offset,
                        offset_remapping,
                    )?;
                }
                trace!("Returning to parent dir ({name:?})");
            }
        };

        Ok(())
    }

    // Visit recursively, starting at the root node
    visit_node(
        "(root)",
        fnt,
        data_table_offs,
        in_file,
        out_file,
        initial_fat_offset,
        &mut offset_remapping,
    )
}

/// Specifies strategies for handling the types of conflicts that can
/// occur in `convert_openingtitle_between_regions()`.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Default)]
pub struct ConvertOpeningTitleBetweenRegionsConflictStrategies {
    /// Different files that are supposed to be merged have conflicting
    /// data. If this is set to `ConflictStrategy::Overwrite`, the
    /// conflict is resolved by prioritizing the region listed first in
    /// the `from_regions` parameter.
    pub file_contents: ConflictStrategy,

    /// One or more of the output filenames (indicated by the
    /// `to_regions` parameter) already exists. If this is set to
    /// `ConflictStrategy::Overwrite`, the conflict is resolved by
    /// overwriting the existing file with the new one.
    pub filenames: ConflictStrategy,
}

/// Read openingTitle.arc from a `Seek+Read`, create a version of it
/// with filenames corrected to match the requested output regions, and
/// write it to a `Seek+Write`. The `Seek+Write` is assumed to be
/// initially empty.
///
/// The default value for `from_regions` is `Region::DEFAULT_ORDER`.
pub fn convert_openingtitle_between_regions<SR: Seek + Read, SW: Seek + Write>(
    mut in_file: SR,
    mut out_file: SW,
    from_regions: Option<&[Region]>,
    to_regions: RegionBitFlags,
    conflict_strategies: &ConvertOpeningTitleBetweenRegionsConflictStrategies,
) -> Result<(), ConvertOpeningTitleBetweenRegionsError> {
    info!("Converting an openingTitle to regions: {to_regions:?}");

    const TOTAL_STEPS: u32 = 9;

    let from_regions = match from_regions {
        Some(regions) => regions,
        None => &Region::DEFAULT_ORDER,
    };

    // Read FNT
    info!("[1/{TOTAL_STEPS}] Reading original FNT...");
    let (mut fnt, data_table_offs) = u8_fnt::read(&mut in_file)?;
    debug!("\n{fnt}");

    // Find existing regional files, make a note of their positions, and
    // delete them
    info!("[2/{TOTAL_STEPS}] Removing all regional files...");
    let all_regional_files =
        remove_regional_files(&mut fnt, RegionBitFlags::from_iter(from_regions))?;

    // Check for conflicts
    if conflict_strategies.file_contents == ConflictStrategy::Fail {
        info!("[3/{TOTAL_STEPS}] Checking for conflicts...");
        check_all_files_for_conflicts(&all_regional_files, data_table_offs, &mut in_file)?;
    }

    // Select the regional files that will be preserved in the output
    // file
    info!("[4/{TOTAL_STEPS}] Selecting regional files...");
    let regional_files = select_regional_files(&all_regional_files, from_regions)?;
    debug!("\n{fnt}");

    // Add new filenames as requested by the user
    info!("[5/{TOTAL_STEPS}] Adding new regional filenames...");
    add_new_filenames(
        &mut fnt,
        &regional_files,
        to_regions,
        conflict_strategies.filenames,
    )?;
    debug!("\n{fnt}");

    // We don't yet have the correct file data offsets in the FNT, but
    // its length won't change when we fill those in.
    // So we can serialize it in memory to check what its length will be
    info!("[6/{TOTAL_STEPS}] Predicting size of new FNT...");
    let mut tmp_cursor = Cursor::new(Vec::new());
    u8_fnt::write(&mut tmp_cursor, &fnt)?;
    let fnt_length = tmp_cursor.into_inner().len();
    info!("...new FNT size will be {fnt_length:#x}");

    // Write nulls to reserve space
    info!("[7/{TOTAL_STEPS}] Writing nulls to reserve space for FNT...");
    out_file.write_all(&vec![0; fnt_length])?;

    // Write the FAT and update offsets in the FNT
    info!("[8/{TOTAL_STEPS}] Building new FAT and updating FNT...");
    build_new_fat(&mut fnt, data_table_offs, &mut in_file, &mut out_file)?;
    debug!("\n{fnt}");

    // Go back and write the real FNT
    info!("[9/{TOTAL_STEPS}] Writing final FNT...");
    out_file.seek(SeekFrom::Start(0))?;
    u8_fnt::write(&mut out_file, &fnt)?;

    info!("Done switching regions!");
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unnecessary_wraps)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    /// Helper function to make an openingTitle FNT for a particular set
    /// of regions, with a provided suite of file nodes
    fn make_openingtitle_fnt(regions: RegionBitFlags, regional_files: &RegionalFiles) -> U8Node {
        let mut anim = U8FolderNode::new();
        let mut blyt = U8FolderNode::new();

        for region in regions {
            anim.extend([
                (
                    ALL_FILENAMES[region.into()].in_press_brlan.to_owned(),
                    U8Node::File(regional_files.in_press_brlan.clone()),
                ),
                (
                    ALL_FILENAMES[region.into()].in_title_brlan.to_owned(),
                    U8Node::File(regional_files.in_title_brlan.clone()),
                ),
                (
                    ALL_FILENAMES[region.into()].loop_press_brlan.to_owned(),
                    U8Node::File(regional_files.loop_press_brlan.clone()),
                ),
                (
                    ALL_FILENAMES[region.into()].out_press_brlan.to_owned(),
                    U8Node::File(regional_files.out_press_brlan.clone()),
                ),
            ]);
            blyt.insert(
                ALL_FILENAMES[region.into()].brlyt.to_owned(),
                U8Node::File(regional_files.brlyt.clone()),
            );
        }

        U8Node::Folder(U8FolderNode::from([(
            "arc".to_owned(),
            U8Node::Folder(U8FolderNode::from([
                ("anim".to_owned(), U8Node::Folder(anim)),
                ("blyt".to_owned(), U8Node::Folder(blyt)),
            ])),
        )]))
    }

    /// Helper function to make a
    /// `HashMap<Region, OptionalNamedRegionalFiles>` for a particular
    /// set of regions, with a provided suite of file nodes
    fn make_hash_map_to_optional_named_regional_files(
        regions: RegionBitFlags,
        regional_files: &RegionalFiles,
    ) -> HashMap<Region, OptionalNamedRegionalFiles> {
        let mut map = HashMap::new();

        for region in regions {
            let region_name = region.into();
            map.insert(
                region,
                OptionalNamedRegionalFiles {
                    in_press_brlan: Some(NamedU8FileNode {
                        node: regional_files.in_press_brlan.clone(),
                        filename: ALL_FILENAMES[region_name].in_press_brlan.to_owned(),
                    }),
                    in_title_brlan: Some(NamedU8FileNode {
                        node: regional_files.in_title_brlan.clone(),
                        filename: ALL_FILENAMES[region_name].in_title_brlan.to_owned(),
                    }),
                    loop_press_brlan: Some(NamedU8FileNode {
                        node: regional_files.loop_press_brlan.clone(),
                        filename: ALL_FILENAMES[region_name].loop_press_brlan.to_owned(),
                    }),
                    out_press_brlan: Some(NamedU8FileNode {
                        node: regional_files.out_press_brlan.clone(),
                        filename: ALL_FILENAMES[region_name].out_press_brlan.to_owned(),
                    }),
                    brlyt: Some(NamedU8FileNode {
                        node: regional_files.brlyt.clone(),
                        filename: ALL_FILENAMES[region_name].brlyt.to_owned(),
                    }),
                },
            );
        }

        map
    }

    mod remove_regional_files {
        use super::*;

        #[test]
        fn test_empty() -> TestResult {
            let mut root = U8Node::Folder(U8FolderNode::new());
            let regional_files = remove_regional_files(&mut root, RegionBitFlags::from(Region::P));
            assert!(regional_files.is_err());
            Ok(())
        }

        #[test]
        fn test_simple() -> TestResult {
            let regional_files = RegionalFiles {
                in_press_brlan: U8FileNode {
                    offset: 0x1,
                    size: 0x1,
                },
                in_title_brlan: U8FileNode {
                    offset: 0x2,
                    size: 0x2,
                },
                loop_press_brlan: U8FileNode {
                    offset: 0x3,
                    size: 0x3,
                },
                out_press_brlan: U8FileNode {
                    offset: 0x4,
                    size: 0x4,
                },
                brlyt: U8FileNode {
                    offset: 0x5,
                    size: 0x5,
                },
            };

            // Input FNT has files for P, E, and J...
            let mut root =
                make_openingtitle_fnt(Region::P | Region::E | Region::J, &regional_files);

            // ...plus some other random file (why not)...
            get_mut_anim_folder(&mut root)?.insert(
                "some other random thing".to_owned(),
                U8Node::File(U8FileNode {
                    offset: 0x10,
                    size: 0x10,
                }),
            );

            // ...we remove the P and E files...
            let removed_files = remove_regional_files(&mut root, Region::P | Region::E)?;

            // ...which leaves the J files and the other one...
            let mut expected_out_root =
                make_openingtitle_fnt(RegionBitFlags::from(Region::J), &regional_files);
            get_mut_anim_folder(&mut expected_out_root)?.insert(
                "some other random thing".to_owned(),
                U8Node::File(U8FileNode {
                    offset: 0x10,
                    size: 0x10,
                }),
            );
            assert_eq!(root, expected_out_root);

            // ...and then the P and E files were returned
            assert_eq!(
                removed_files,
                make_hash_map_to_optional_named_regional_files(
                    Region::P | Region::E,
                    &regional_files
                )
            );

            Ok(())
        }
    }

    mod check_file_pair_for_conflicts {
        use super::*;

        #[test]
        fn test_nodes_identical() -> TestResult {
            let old = NamedU8FileNode {
                node: U8FileNode {
                    offset: 8,
                    size: 10,
                },
                filename: "old".to_owned(),
            };
            let new = NamedU8FileNode {
                node: U8FileNode {
                    offset: 8,
                    size: 10,
                },
                filename: "new".to_owned(),
            };

            let mut cursor = Cursor::new((0..255).collect::<Vec<u8>>());

            let mut current_file = None;
            let mut current_hash = 0;

            // Check both files -- no errors should be returned
            check_file_pair_for_conflicts(
                &mut current_file,
                &mut current_hash,
                &old,
                10,
                &mut cursor,
            )?;
            check_file_pair_for_conflicts(
                &mut current_file,
                &mut current_hash,
                &new,
                10,
                &mut cursor,
            )?;

            // Since the nodes (offsets/sizes) are identical, it
            // shouldn't've needed to calculate any hashes
            assert_eq!(current_hash, 0);
            Ok(())
        }

        #[test]
        fn test_data_identical() -> TestResult {
            let old = NamedU8FileNode {
                node: U8FileNode { offset: 0, size: 8 },
                filename: "old".to_owned(),
            };
            let new = NamedU8FileNode {
                node: U8FileNode { offset: 8, size: 8 },
                filename: "new".to_owned(),
            };

            // (three "X"s as a placeholder for the U8 FNT)
            let data = b"XXXAAAABBBBAAAABBBBCCCCDDDD";
            let mut cursor = Cursor::new(data.to_vec());

            let mut current_file = None;
            let mut current_hash = 0;

            // Check both files -- no errors should be returned
            check_file_pair_for_conflicts(
                &mut current_file,
                &mut current_hash,
                &old,
                3,
                &mut cursor,
            )?;
            check_file_pair_for_conflicts(
                &mut current_file,
                &mut current_hash,
                &new,
                3,
                &mut cursor,
            )?;

            // The offsets were different, so it should've calculated
            // hashes in order to compare the data
            assert_ne!(current_hash, 0);
            Ok(())
        }

        #[test]
        fn test_data_different() -> TestResult {
            let old = NamedU8FileNode {
                node: U8FileNode { offset: 0, size: 8 },
                filename: "old".to_owned(),
            };
            let new = NamedU8FileNode {
                node: U8FileNode { offset: 8, size: 8 },
                filename: "new".to_owned(),
            };

            // (three "X"s as a placeholder for the U8 FNT)
            let data = b"XXXAAAABBBBCCCCDDDDAAAABBBB";
            let mut cursor = Cursor::new(data.to_vec());

            let mut current_file = None;
            let mut current_hash = 0;

            // Check both files -- second one should return an error
            check_file_pair_for_conflicts(
                &mut current_file,
                &mut current_hash,
                &old,
                3,
                &mut cursor,
            )?;
            assert!(check_file_pair_for_conflicts(
                &mut current_file,
                &mut current_hash,
                &new,
                3,
                &mut cursor
            )
            .is_err());

            // And it should've calculated the hash in order to check
            assert_ne!(current_hash, 0);
            Ok(())
        }
    }

    mod check_all_files_for_conflicts {
        use super::*;

        #[test]
        fn test_no_conflicts() -> TestResult {
            let mut files = make_hash_map_to_optional_named_regional_files(
                RegionBitFlags::from(Region::P),
                &RegionalFiles {
                    in_press_brlan: U8FileNode {
                        offset: 0x1,
                        size: 0x1,
                    },
                    in_title_brlan: U8FileNode {
                        offset: 0x2,
                        size: 0x2,
                    },
                    loop_press_brlan: U8FileNode {
                        offset: 0x3,
                        size: 0x3,
                    },
                    out_press_brlan: U8FileNode {
                        offset: 0x4,
                        size: 0x4,
                    },
                    brlyt: U8FileNode {
                        offset: 0x5,
                        size: 0x5,
                    },
                },
            );
            files.extend(make_hash_map_to_optional_named_regional_files(
                RegionBitFlags::from(Region::E),
                &RegionalFiles {
                    in_press_brlan: U8FileNode {
                        offset: 0x11,
                        size: 0x1,
                    },
                    in_title_brlan: U8FileNode {
                        offset: 0x12,
                        size: 0x2,
                    },
                    loop_press_brlan: U8FileNode {
                        offset: 0x13,
                        size: 0x3,
                    },
                    out_press_brlan: U8FileNode {
                        offset: 0x14,
                        size: 0x4,
                    },
                    brlyt: U8FileNode {
                        offset: 0x15,
                        size: 0x5,
                    },
                },
            ));
            let files = files;

            let data = b"0123456789abcdef0123456789abcdef";
            let mut cursor = Cursor::new(data.to_vec());

            check_all_files_for_conflicts(&files, 0, &mut cursor)?;

            Ok(())
        }

        #[test]
        fn test_conflicts() -> TestResult {
            let mut files = make_hash_map_to_optional_named_regional_files(
                RegionBitFlags::from(Region::P),
                &RegionalFiles {
                    in_press_brlan: U8FileNode {
                        offset: 0x1,
                        size: 0x1,
                    },
                    in_title_brlan: U8FileNode {
                        offset: 0x2,
                        size: 0x2,
                    },
                    loop_press_brlan: U8FileNode {
                        offset: 0x3,
                        size: 0x3,
                    },
                    out_press_brlan: U8FileNode {
                        offset: 0x4,
                        size: 0x4,
                    },
                    brlyt: U8FileNode {
                        offset: 0x5,
                        size: 0x5,
                    },
                },
            );
            files.extend(make_hash_map_to_optional_named_regional_files(
                RegionBitFlags::from(Region::E),
                &RegionalFiles {
                    in_press_brlan: U8FileNode {
                        offset: 0x11,
                        size: 0x1,
                    },
                    in_title_brlan: U8FileNode {
                        offset: 0x12,
                        size: 0x2,
                    },
                    loop_press_brlan: U8FileNode {
                        offset: 0x13,
                        size: 0x3,
                    },
                    out_press_brlan: U8FileNode {
                        offset: 0x14,
                        size: 0x4,
                    },
                    brlyt: U8FileNode {
                        offset: 0x15,
                        size: 0x5,
                    },
                },
            ));
            let files = files;

            let data = b"0123456789abcdef0123_56789abcdef";
            let mut cursor = Cursor::new(data.to_vec());

            assert!(check_all_files_for_conflicts(&files, 0, &mut cursor).is_err());

            Ok(())
        }
    }

    mod select_regional_files {
        use super::*;

        #[test]
        fn test_simple() -> TestResult {
            let mut files = make_hash_map_to_optional_named_regional_files(
                RegionBitFlags::from(Region::K),
                &RegionalFiles {
                    in_press_brlan: U8FileNode {
                        offset: 0x1,
                        size: 0x1,
                    },
                    in_title_brlan: U8FileNode {
                        offset: 0x2,
                        size: 0x2,
                    },
                    loop_press_brlan: U8FileNode {
                        offset: 0x3,
                        size: 0x3,
                    },
                    out_press_brlan: U8FileNode {
                        offset: 0x4,
                        size: 0x4,
                    },
                    brlyt: U8FileNode {
                        offset: 0x5,
                        size: 0x5,
                    },
                },
            );
            files.extend(make_hash_map_to_optional_named_regional_files(
                RegionBitFlags::from(Region::E),
                &RegionalFiles {
                    in_press_brlan: U8FileNode {
                        offset: 0x11,
                        size: 0x1,
                    },
                    in_title_brlan: U8FileNode {
                        offset: 0x12,
                        size: 0x2,
                    },
                    loop_press_brlan: U8FileNode {
                        offset: 0x13,
                        size: 0x3,
                    },
                    out_press_brlan: U8FileNode {
                        offset: 0x14,
                        size: 0x4,
                    },
                    brlyt: U8FileNode {
                        offset: 0x15,
                        size: 0x5,
                    },
                },
            ));
            files.get_mut(&Region::K).unwrap().in_title_brlan = None;
            let files = files;

            // Prioritize K over E (opposite of the default)
            // K is missing in_title_brlan, so it should get that one
            // from E, and the rest from K
            let selected = select_regional_files(&files, &[Region::K, Region::E])?;

            assert_eq!(
                selected,
                RegionalFiles {
                    in_press_brlan: U8FileNode {
                        offset: 0x1,
                        size: 0x1
                    },
                    in_title_brlan: U8FileNode {
                        offset: 0x12,
                        size: 0x2
                    },
                    loop_press_brlan: U8FileNode {
                        offset: 0x3,
                        size: 0x3
                    },
                    out_press_brlan: U8FileNode {
                        offset: 0x4,
                        size: 0x4
                    },
                    brlyt: U8FileNode {
                        offset: 0x5,
                        size: 0x5
                    },
                }
            );

            Ok(())
        }

        #[test]
        fn test_missing_files() -> TestResult {
            let mut files = make_hash_map_to_optional_named_regional_files(
                RegionBitFlags::from(Region::K),
                &RegionalFiles {
                    in_press_brlan: U8FileNode {
                        offset: 0x1,
                        size: 0x1,
                    },
                    in_title_brlan: U8FileNode {
                        offset: 0x2,
                        size: 0x2,
                    },
                    loop_press_brlan: U8FileNode {
                        offset: 0x3,
                        size: 0x3,
                    },
                    out_press_brlan: U8FileNode {
                        offset: 0x4,
                        size: 0x4,
                    },
                    brlyt: U8FileNode {
                        offset: 0x5,
                        size: 0x5,
                    },
                },
            );
            files.extend(make_hash_map_to_optional_named_regional_files(
                RegionBitFlags::from(Region::E),
                &RegionalFiles {
                    in_press_brlan: U8FileNode {
                        offset: 0x11,
                        size: 0x1,
                    },
                    in_title_brlan: U8FileNode {
                        offset: 0x12,
                        size: 0x2,
                    },
                    loop_press_brlan: U8FileNode {
                        offset: 0x13,
                        size: 0x3,
                    },
                    out_press_brlan: U8FileNode {
                        offset: 0x14,
                        size: 0x4,
                    },
                    brlyt: U8FileNode {
                        offset: 0x15,
                        size: 0x5,
                    },
                },
            ));
            files.get_mut(&Region::K).unwrap().in_title_brlan = None;
            files.get_mut(&Region::E).unwrap().in_title_brlan = None;
            let files = files;

            // in_title_brlan is missing from all regions, so this
            // should fail
            assert!(select_regional_files(&files, &[Region::K, Region::E]).is_err());

            Ok(())
        }
    }

    mod add_new_filenames {
        use super::*;

        #[test]
        fn test_individual() -> TestResult {
            let mut root = U8Node::Folder(U8FolderNode::from([(
                "arc".to_owned(),
                U8Node::Folder(U8FolderNode::from([
                    ("anim".to_owned(), U8Node::Folder(U8FolderNode::new())),
                    ("blyt".to_owned(), U8Node::Folder(U8FolderNode::new())),
                ])),
            )]));
            let regional_files = RegionalFiles {
                in_press_brlan: U8FileNode {
                    offset: 0x1,
                    size: 0x1,
                },
                in_title_brlan: U8FileNode {
                    offset: 0x2,
                    size: 0x2,
                },
                loop_press_brlan: U8FileNode {
                    offset: 0x3,
                    size: 0x3,
                },
                out_press_brlan: U8FileNode {
                    offset: 0x4,
                    size: 0x4,
                },
                brlyt: U8FileNode {
                    offset: 0x5,
                    size: 0x5,
                },
            };

            add_new_filenames(
                &mut root,
                &regional_files,
                RegionBitFlags::from(Region::K),
                ConflictStrategy::default(),
            )?;

            assert_eq!(
                root,
                make_openingtitle_fnt(RegionBitFlags::from(Region::K), &regional_files)
            );
            Ok(())
        }

        #[test]
        fn test_region_free() -> TestResult {
            let mut root = U8Node::Folder(U8FolderNode::from([(
                "arc".to_owned(),
                U8Node::Folder(U8FolderNode::from([
                    ("anim".to_owned(), U8Node::Folder(U8FolderNode::new())),
                    ("blyt".to_owned(), U8Node::Folder(U8FolderNode::new())),
                ])),
            )]));
            let regional_files = RegionalFiles {
                in_press_brlan: U8FileNode {
                    offset: 0x1,
                    size: 0x1,
                },
                in_title_brlan: U8FileNode {
                    offset: 0x2,
                    size: 0x2,
                },
                loop_press_brlan: U8FileNode {
                    offset: 0x3,
                    size: 0x3,
                },
                out_press_brlan: U8FileNode {
                    offset: 0x4,
                    size: 0x4,
                },
                brlyt: U8FileNode {
                    offset: 0x5,
                    size: 0x5,
                },
            };

            add_new_filenames(
                &mut root,
                &regional_files,
                RegionBitFlags::ALL,
                ConflictStrategy::default(),
            )?;

            assert_eq!(
                get_anim_folder(&root)?.len(),
                4 * Region::DEFAULT_ORDER.len()
            );
            assert_eq!(get_blyt_folder(&root)?.len(), Region::DEFAULT_ORDER.len());

            Ok(())
        }

        #[test]
        fn test_forbidden_conflicts() -> TestResult {
            let mut root = U8Node::Folder(U8FolderNode::from([(
                "arc".to_owned(),
                U8Node::Folder(U8FolderNode::from([
                    (
                        "anim".to_owned(),
                        U8Node::Folder(U8FolderNode::from([(
                            ALL_FILENAMES["W"].loop_press_brlan.to_owned(),
                            U8Node::File(U8FileNode { offset: 0, size: 0 }),
                        )])),
                    ),
                    ("blyt".to_owned(), U8Node::Folder(U8FolderNode::new())),
                ])),
            )]));
            let regional_files = RegionalFiles {
                in_press_brlan: U8FileNode {
                    offset: 0x1,
                    size: 0x1,
                },
                in_title_brlan: U8FileNode {
                    offset: 0x2,
                    size: 0x2,
                },
                loop_press_brlan: U8FileNode {
                    offset: 0x3,
                    size: 0x3,
                },
                out_press_brlan: U8FileNode {
                    offset: 0x4,
                    size: 0x4,
                },
                brlyt: U8FileNode {
                    offset: 0x5,
                    size: 0x5,
                },
            };

            assert!(add_new_filenames(
                &mut root,
                &regional_files,
                RegionBitFlags::from(Region::W),
                ConflictStrategy::Fail
            )
            .is_err());

            Ok(())
        }

        #[test]
        fn test_allowed_conflicts() -> TestResult {
            let mut root = U8Node::Folder(U8FolderNode::from([(
                "arc".to_owned(),
                U8Node::Folder(U8FolderNode::from([
                    (
                        "anim".to_owned(),
                        U8Node::Folder(U8FolderNode::from([(
                            ALL_FILENAMES["W"].loop_press_brlan.to_owned(),
                            U8Node::File(U8FileNode { offset: 0, size: 0 }),
                        )])),
                    ),
                    ("blyt".to_owned(), U8Node::Folder(U8FolderNode::new())),
                ])),
            )]));
            let regional_files = RegionalFiles {
                in_press_brlan: U8FileNode {
                    offset: 0x1,
                    size: 0x1,
                },
                in_title_brlan: U8FileNode {
                    offset: 0x2,
                    size: 0x2,
                },
                loop_press_brlan: U8FileNode {
                    offset: 0x3,
                    size: 0x3,
                },
                out_press_brlan: U8FileNode {
                    offset: 0x4,
                    size: 0x4,
                },
                brlyt: U8FileNode {
                    offset: 0x5,
                    size: 0x5,
                },
            };

            add_new_filenames(
                &mut root,
                &regional_files,
                RegionBitFlags::from(Region::W),
                ConflictStrategy::Overwrite,
            )?;

            assert_eq!(
                root,
                make_openingtitle_fnt(RegionBitFlags::from(Region::W), &regional_files)
            );

            Ok(())
        }
    }

    mod build_new_fat {
        use super::*;

        #[test]
        fn test_empty() -> TestResult {
            let mut root = U8Node::Folder(U8FolderNode::new());
            let mut in_buf = Cursor::new(Vec::new());

            let mut out_buf = Cursor::new(Vec::new());
            build_new_fat(&mut root, 0, &mut in_buf, &mut out_buf)?;

            assert_eq!(out_buf.stream_position()?, 0);
            Ok(())
        }

        #[test]
        fn test_simple() -> TestResult {
            let mut root = U8Node::Folder(U8FolderNode::from([
                (
                    "a".to_owned(),
                    U8Node::File(U8FileNode {
                        offset: 0x40,
                        size: 0x10,
                    }),
                ),
                (
                    "b".to_owned(),
                    U8Node::File(U8FileNode {
                        offset: 0x20,
                        size: 0x10,
                    }),
                ),
                (
                    "c".to_owned(),
                    U8Node::File(U8FileNode {
                        offset: 0x10,
                        size: 0x10,
                    }),
                ),
                (
                    "d".to_owned(),
                    U8Node::File(U8FileNode {
                        offset: 0x40,
                        size: 0x10,
                    }),
                ),
                (
                    "e".to_owned(),
                    U8Node::File(U8FileNode {
                        offset: 0x30,
                        size: 0x10,
                    }),
                ),
                (
                    "f".to_owned(),
                    U8Node::File(U8FileNode {
                        offset: 0x20,
                        size: 0x10,
                    }),
                ),
            ]));
            let mut in_buf = Cursor::new((0..255).collect::<Vec<u8>>());

            let mut out_buf = Cursor::new(Vec::new());
            build_new_fat(&mut root, 0, &mut in_buf, &mut out_buf)?;

            assert_eq!(
                root,
                U8Node::Folder(U8FolderNode::from([
                    (
                        "a".to_owned(),
                        U8Node::File(U8FileNode {
                            offset: 0x00,
                            size: 0x10
                        })
                    ),
                    (
                        "b".to_owned(),
                        U8Node::File(U8FileNode {
                            offset: 0x20,
                            size: 0x10
                        })
                    ),
                    (
                        "c".to_owned(),
                        U8Node::File(U8FileNode {
                            offset: 0x40,
                            size: 0x10
                        })
                    ),
                    (
                        "d".to_owned(),
                        U8Node::File(U8FileNode {
                            offset: 0x00,
                            size: 0x10
                        })
                    ),
                    (
                        "e".to_owned(),
                        U8Node::File(U8FileNode {
                            offset: 0x60,
                            size: 0x10
                        })
                    ),
                    (
                        "f".to_owned(),
                        U8Node::File(U8FileNode {
                            offset: 0x20,
                            size: 0x10
                        })
                    ),
                ]))
            );

            assert_eq!(
                &out_buf.into_inner(),
                concat_bytes!(
                    b"\x40\x41\x42\x43\x44\x45\x46\x47\x48\x49\x4a\x4b\x4c\x4d\x4e\x4f",
                    b"\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
                    b"\x20\x21\x22\x23\x24\x25\x26\x27\x28\x29\x2a\x2b\x2c\x2d\x2e\x2f",
                    b"\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
                    b"\x10\x11\x12\x13\x14\x15\x16\x17\x18\x19\x1a\x1b\x1c\x1d\x1e\x1f",
                    b"\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
                    b"\x30\x31\x32\x33\x34\x35\x36\x37\x38\x39\x3a\x3b\x3c\x3d\x3e\x3f",
                )
            );
            Ok(())
        }
    }

    mod convert_openingtitle_between_regions {
        use super::*;

        #[test]
        fn test_individual() -> TestResult {
            let mut in_root = make_openingtitle_fnt(
                RegionBitFlags::from(Region::W),
                &RegionalFiles {
                    in_press_brlan: U8FileNode {
                        offset: 0x00,
                        size: 0x8,
                    },
                    in_title_brlan: U8FileNode {
                        offset: 0x20,
                        size: 0x8,
                    },
                    loop_press_brlan: U8FileNode {
                        offset: 0x40,
                        size: 0x8,
                    },
                    out_press_brlan: U8FileNode {
                        offset: 0x60,
                        size: 0x8,
                    },
                    brlyt: U8FileNode {
                        offset: 0xa0,
                        size: 0x8,
                    },
                },
            );

            get_mut_anim_folder(&mut in_root)?.insert(
                "something else".to_owned(),
                U8Node::File(U8FileNode {
                    offset: 0x80,
                    size: 0x8,
                }),
            );

            let mut in_buf = Cursor::new(Vec::new());
            u8_fnt::write(&mut in_buf, &in_root)?;
            in_buf.write_all(concat_bytes!(
                b"INPBRLAN\0\0\0\0\0\0\0\0",
                b"\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
                b"INTBRLAN\0\0\0\0\0\0\0\0",
                b"\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
                b"LPPBRLAN\0\0\0\0\0\0\0\0",
                b"\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
                b"OTPBRLAN\0\0\0\0\0\0\0\0",
                b"\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
                b"SMTHELSE\0\0\0\0\0\0\0\0",
                b"\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
                b"THEBRLYT\0\0\0\0\0\0\0\0",
                b"\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
            ))?;

            in_buf.seek(SeekFrom::Start(0))?;
            let mut out_buf = Cursor::new(Vec::new());

            convert_openingtitle_between_regions(
                &mut in_buf,
                &mut out_buf,
                None,
                RegionBitFlags::from(Region::J),
                &ConvertOpeningTitleBetweenRegionsConflictStrategies::default(),
            )?;

            out_buf.seek(SeekFrom::Start(0))?;
            let (out_root, out_data_offset) = u8_fnt::read(&mut out_buf)?;

            let mut expected_out_root = make_openingtitle_fnt(
                RegionBitFlags::from(Region::J),
                &RegionalFiles {
                    in_press_brlan: U8FileNode {
                        offset: 0x00,
                        size: 0x8,
                    },
                    in_title_brlan: U8FileNode {
                        offset: 0x20,
                        size: 0x8,
                    },
                    loop_press_brlan: U8FileNode {
                        offset: 0x40,
                        size: 0x8,
                    },
                    out_press_brlan: U8FileNode {
                        offset: 0x60,
                        size: 0x8,
                    },
                    brlyt: U8FileNode {
                        offset: 0xa0,
                        size: 0x8,
                    },
                },
            );

            get_mut_anim_folder(&mut expected_out_root)?.insert(
                "something else".to_owned(),
                U8Node::File(U8FileNode {
                    offset: 0x80,
                    size: 0x8,
                }),
            );

            assert_eq!(out_root, expected_out_root);
            assert_eq!(out_data_offset, 0x160);
            Ok(())
        }

        #[test]
        fn test_region_free() -> TestResult {
            let in_root = U8Node::Folder(U8FolderNode::from([(
                "arc".to_owned(),
                U8Node::Folder(U8FolderNode::from([
                    (
                        "anim".to_owned(),
                        U8Node::Folder(U8FolderNode::from([
                            (
                                ALL_FILENAMES["P"].in_press_brlan.to_owned(),
                                U8Node::File(U8FileNode {
                                    offset: 0x00,
                                    size: 0x8,
                                }),
                            ),
                            (
                                ALL_FILENAMES["E"].in_title_brlan.to_owned(),
                                U8Node::File(U8FileNode {
                                    offset: 0x20,
                                    size: 0x8,
                                }),
                            ),
                            (
                                ALL_FILENAMES["J"].loop_press_brlan.to_owned(),
                                U8Node::File(U8FileNode {
                                    offset: 0x40,
                                    size: 0x8,
                                }),
                            ),
                            (
                                ALL_FILENAMES["K"].out_press_brlan.to_owned(),
                                U8Node::File(U8FileNode {
                                    offset: 0x60,
                                    size: 0x8,
                                }),
                            ),
                        ])),
                    ),
                    (
                        "blyt".to_owned(),
                        U8Node::Folder(U8FolderNode::from([(
                            ALL_FILENAMES["W"].brlyt.to_owned(),
                            U8Node::File(U8FileNode {
                                offset: 0x80,
                                size: 0x8,
                            }),
                        )])),
                    ),
                ])),
            )]));

            let mut in_buf = Cursor::new(Vec::new());
            u8_fnt::write(&mut in_buf, &in_root)?;
            in_buf.write_all(concat_bytes!(
                b"INPBRLAN\0\0\0\0\0\0\0\0",
                b"\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
                b"INTBRLAN\0\0\0\0\0\0\0\0",
                b"\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
                b"LPPBRLAN\0\0\0\0\0\0\0\0",
                b"\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
                b"OTPBRLAN\0\0\0\0\0\0\0\0",
                b"\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
                b"THEBRLYT\0\0\0\0\0\0\0\0",
                b"\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
            ))?;

            in_buf.seek(SeekFrom::Start(0))?;
            let mut out_buf = Cursor::new(Vec::new());

            convert_openingtitle_between_regions(
                &mut in_buf,
                &mut out_buf,
                None,
                RegionBitFlags::ALL,
                &ConvertOpeningTitleBetweenRegionsConflictStrategies::default(),
            )?;

            out_buf.seek(SeekFrom::Start(0))?;
            let (out_root, out_data_offset) = u8_fnt::read(&mut out_buf)?;

            let anim_folder = get_anim_folder(&out_root)?;
            let blyt_folder = get_blyt_folder(&out_root)?;
            for region in &Region::DEFAULT_ORDER {
                let region_name = region.into();
                assert!(anim_folder
                    .get(ALL_FILENAMES[region_name].in_press_brlan)
                    .is_some());
                assert!(anim_folder
                    .get(ALL_FILENAMES[region_name].in_title_brlan)
                    .is_some());
                assert!(anim_folder
                    .get(ALL_FILENAMES[region_name].loop_press_brlan)
                    .is_some());
                assert!(anim_folder
                    .get(ALL_FILENAMES[region_name].out_press_brlan)
                    .is_some());
                assert!(blyt_folder.get(ALL_FILENAMES[region_name].brlyt).is_some());
            }
            assert_eq!(out_data_offset, 0x580);
            Ok(())
        }
    }
}
