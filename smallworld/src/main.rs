#![doc = include_str!("../../README.md")]

use std::fs::File;
use std::io::{Cursor, Read, Seek, Write};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::Parser;
use log::{debug, trace};

use libsmallworld as lib;

#[derive(Parser, Debug)]
#[clap(author, version, about = "A little tool to create region-free \
openingTitle.arc files for New Super Mario Bros. Wii, or to convert \
them from one region to another.

Run with `-h` for \"short\" help, or `--help` for \"long\" help.",
long_about = None)]
struct Args {
    // NOTE: Clap uses the documentation comments below to create the
    // auto-generated `--help` output, which is why they're worded a bit
    // oddly
    /// Input filename
    input_file: PathBuf,

    /// Output filename [default: overwrite the input file]
    #[clap(short, long)]
    output_file: Option<PathBuf>,

    /// The regions to convert from, in order of priority
    ///
    /// Files from regions not listed here will be entirely ignored.
    /// Furthermore, if `--ignore-conflicts` is specified, conflicts
    /// will be resolved in favor of the regions that come first in this
    /// list. It's OK if the input arc doesn't have all of the regions
    /// listed here -- that is, the default ("all") will work for every
    /// valid openingTitle.arc.
    ///
    /// For example, "--from E" means "use only the US files (and fail
    /// if any are missing)". "--from E,J" (without
    /// `--ignore-conflicts`) means "use US or JP files, whichever is
    /// available, and fail if both are missing or if they conflict."
    /// `--from E,J --ignore-conflicts` means "use US or JP files,
    /// whichever is available; if they conflict, favor the US files
    /// (and fail if both are missing)."
    ///
    /// The default is "all", which is shorthand for "P,E,J,K,W,C".
    #[clap(long, value_parser, default_value = "all")]
    from: String,

    /// The regions to include filenames for in the output
    ///
    /// If you're not targeting every region, you can save a few bytes
    /// by omitting some. Unlike `--from`, the order in which you list
    /// the regions doesn't matter.
    ///
    /// The default is "all", which is shorthand for "P,E,J,K,W,C".
    ///
    /// smallworld will fail if the target filenames already exist and
    /// were omitted from `--from`, unless `--ignore-conflicts` was
    /// specified.
    #[clap(long, value_parser, default_value = "all")]
    to: String,

    /// Generate the output file even if conflicts are found
    ///
    /// There are two types of conflicts that can occur: (1) two files
    /// from regions listed in `--from` are found to have different
    /// data, or (2) a file from a region listed in `--to` already
    /// exists in the arc.
    ///
    /// If either of these happens, normally smallworld will err on the
    /// side of avoiding possible data loss, and refuse to continue.
    /// Pass this flag to make it proceed anyway. Type-(1) conflicts are
    /// then resolved in favor of whichever region is listed first in
    /// `--from`, and type-(2) conflicts are resolved by overwriting the
    /// existing file(s).
    #[clap(long, action)]
    ignore_conflicts: bool,

    #[clap(flatten)]
    verbose: clap_verbosity_flag::Verbosity,
}

/// Read a string containing a list of regions (e.g. `"P,J,C"`) or
/// `"all"` (meaning all regions, in the default order), and return a
/// `Vec` with the `Region`s.
fn read_region_list_str(arg: &str) -> Result<Vec<lib::Region>> {
    if arg.to_lowercase() == "all" {
        return Ok(lib::Region::DEFAULT_ORDER.to_vec());
    }

    let mut result = Vec::new();
    for item in arg.split(',') {
        let region: lib::Region = item.parse()?;
        if result.contains(&region) {
            bail!(r#"region "{item}" specified more than once"#);
        }
        result.push(region);
    }

    Ok(result)
}

pub trait SeekRead: Seek + Read {}
impl<T: Seek + Read> SeekRead for T {}
pub trait SeekWrite: Seek + Write {}
impl<T: Seek + Write> SeekWrite for T {}

/// Run a function that reads from one file-path and writes to another,
/// efficiently. Will write directly to the output file if the two paths
/// are distinct; otherwise it will buffer the output data in memory and
/// write it into the file afterward.
pub fn run_file_conversion_function(
    input_filepath: &Path,
    output_filepath: &Path,
    conversion_function: impl Fn(&mut dyn SeekRead, &mut dyn SeekWrite) -> Result<()>,
) -> Result<()> {
    // Open input file
    let mut in_file = File::open(&input_filepath)
        .with_context(|| format!("couldn't open input file \"{}\"", input_filepath.display()))?;

    // If the output file can be proven distinct from the input file,
    // we can optimize by writing our output to it directly
    let paths_definitely_distinct = if let Ok(input_canon) = input_filepath.canonicalize() {
        if let Ok(output_canon) = output_filepath.canonicalize() {
            input_canon != output_canon
        } else {
            false
        }
    } else {
        false
    };
    trace!(
        "Paths {} definitely distinct",
        if paths_definitely_distinct {
            "are"
        } else {
            "are NOT"
        }
    );

    if paths_definitely_distinct {
        // We believe that the input and output files are different, so
        // it should be safe to open the output file before we've read
        // the input file.

        // Open the output file
        let mut out_file = File::create(&output_filepath).with_context(|| {
            format!(
                "couldn't open output file \"{}\"",
                output_filepath.display()
            )
        })?;

        // Write directly to it
        conversion_function(&mut in_file, &mut out_file)?;
    } else {
        // The input and output files may be the same.

        // Create a buffer in memory
        let mut buf: Cursor<Vec<u8>> = Cursor::new(Vec::new());

        // Write to it
        conversion_function(&mut in_file, &mut buf)?;
        trace!(
            "Buffered {:#x} bytes of output file data in memory",
            buf.get_ref().len()
        );

        // Open the output file and write the buffer data to it
        File::create(&output_filepath)
            .with_context(|| {
                format!(
                    "couldn't open output file \"{}\"",
                    output_filepath.display()
                )
            })?
            .write_all(&buf.into_inner())
            .context("couldn't write data to output file")?;
    }

    Ok(())
}

/// Entry-point function (mainly deals with CLI-related logic)
fn main() -> Result<()> {
    let args = Args::parse();

    env_logger::Builder::new()
        .filter_level(args.verbose.log_level_filter())
        .init();

    let input_filepath = &args.input_file;
    debug!(
        "Input filepath: {:?} ({:?})",
        input_filepath,
        input_filepath.canonicalize()
    );

    // If not specified, default output path is the input path (i.e. overwrite)
    let output_filepath = match &args.output_file {
        Some(filename) => filename,
        None => &args.input_file,
    };
    debug!(
        "Output filepath: {:?} ({:?})",
        output_filepath,
        output_filepath.canonicalize()
    );

    let from_regions =
        read_region_list_str(&args.from).context("couldn't read `--from` region list")?;
    let from_regions = Some(&from_regions as &[lib::Region]);

    let to_regions = lib::RegionBitFlags::from_iter(
        read_region_list_str(&args.to).context("couldn't read `--to` region list")?,
    );

    // I don't think this is actually possible, but just in case
    if to_regions == lib::RegionBitFlags::EMPTY {
        bail!("must select at least one output region");
    }

    let conflict_strategy = if args.ignore_conflicts {
        lib::ConflictStrategy::Overwrite
    } else {
        lib::ConflictStrategy::Fail
    };
    let conflict_strategies = lib::ConvertOpeningTitleBetweenRegionsConflictStrategies {
        file_contents: conflict_strategy,
        filenames: conflict_strategy,
    };

    run_file_conversion_function(input_filepath, output_filepath, |in_file, out_file| {
        lib::convert_openingtitle_between_regions(
            in_file,
            out_file,
            from_regions,
            to_regions,
            &conflict_strategies,
        )
        .context("failed to perform region conversion")
    })
}

#[cfg(test)]
#[allow(clippy::unnecessary_wraps)]
mod tests {
    use super::*;

    use assert_fs::{assert::PathAssert, fixture::FileWriteBin, NamedTempFile};

    #[test]
    fn test_read_region_list_str() -> Result<()> {
        use lib::Region::{C, E, K};

        assert_eq!(&read_region_list_str("E")?, &[E]);
        assert_eq!(&read_region_list_str("e,k,c")?, &[E, K, C]);
        assert_eq!(&read_region_list_str("all")?, &lib::Region::DEFAULT_ORDER);
        assert_eq!(&read_region_list_str("ALL")?, &lib::Region::DEFAULT_ORDER);

        assert!(&read_region_list_str("").is_err());
        assert!(&read_region_list_str(",").is_err());
        assert!(&read_region_list_str("test").is_err());
        assert!(&read_region_list_str("e,e").is_err());

        Ok(())
    }

    mod run_file_conversion_function {
        use super::*;

        /// Helper function to serve as a simple "conversion function":
        /// just copies the data and adds 3 to every byte
        pub fn copy_and_add_three_to_every_byte(
            in_file: &mut dyn SeekRead,
            out_file: &mut dyn SeekWrite,
        ) -> Result<()> {
            let mut tmp: [u8; 1] = [0; 1];
            while in_file.read(&mut tmp)? > 0 {
                tmp[0] += 3;
                out_file.write_all(&tmp)?;
            }

            Ok(())
        }

        #[test]
        fn test_distinct() -> Result<()> {
            let in_filepath = NamedTempFile::new("test_in.bin")?;
            in_filepath.write_binary(b"\x00\x01\x02\x03\x04\x05\x06\x07")?;

            let out_filepath = NamedTempFile::new("test_out.bin")?;

            run_file_conversion_function(
                in_filepath.path(),
                out_filepath.path(),
                |in_file, out_file| copy_and_add_three_to_every_byte(in_file, out_file),
            )?;

            out_filepath.assert(b"\x03\x04\x05\x06\x07\x08\x09\x0a" as &[u8]);
            Ok(())
        }

        #[test]
        fn test_overwrite() -> Result<()> {
            let filepath = NamedTempFile::new("test.bin")?;
            filepath.write_binary(b"\x00\x01\x02\x03\x04\x05\x06\x07")?;

            run_file_conversion_function(filepath.path(), filepath.path(), |in_file, out_file| {
                copy_and_add_three_to_every_byte(in_file, out_file)
            })?;

            filepath.assert(b"\x03\x04\x05\x06\x07\x08\x09\x0a" as &[u8]);
            Ok(())
        }
    }
}
