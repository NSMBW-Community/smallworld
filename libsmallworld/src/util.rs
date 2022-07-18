//! Various utility functions.

use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::io::{self, Read, Seek, SeekFrom, Write};

/// Write nulls to the provided `Write`, to pad its position to the next
/// multiple of `alignment`.
pub fn write_zeros_to_align_to<SW: Seek + Write>(
    file: &mut SW,
    alignment: u64,
    relative_to: u64,
) -> Result<(), io::Error> {
    let pos = file.stream_position()?;

    if pos < relative_to {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("trying to align to {alignment:#x} at a position ({pos:#x}) before the relative start of the file ({relative_to:#x})")));
    }
    let pos = pos - relative_to;

    let write_amount = pos.checked_next_multiple_of(alignment).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("couldn't align {pos:#x} to {alignment:#x}"),
        )
    })? - pos;

    if write_amount > 0 {
        file.write_all(&vec![0; write_amount.try_into().unwrap()])?;
    }

    Ok(())
}

/// Read data from one file and write it to another.
pub fn read_from_into<R: Read, W: Write>(
    read_from: &mut R,
    write_to: &mut W,
    amount: usize,
) -> Result<(), io::Error> {
    // It'd be good to write this with more of a streaming approach, but
    // I don't know how to determine the optimal buffer size...
    let mut tmp = vec![0; amount];
    let actual_amount = read_from.read(&mut tmp)?;
    if actual_amount < amount {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            format!("tried to read {amount} bytes, but could only read {actual_amount}"),
        ));
    }
    write_to.write_all(&tmp)?;
    Ok(())
}

/// Write a string to a `Write`, right-aligned with space characters.
/// Specify the amount of characters that've already been written to
/// the line, and the column number the string should be right-aligned
/// to.
///
/// If the string can't fit (e.g. we're at position 5 and want to
/// right-align to column 10, but the string is 8 characters long), the
/// string will be written anyway after zero spaces, overflowing the
/// requested column number. `amount_written` will be updated correctly
/// in this case, so be sure to check it if you need to keep track of
/// the current column number!
pub fn write_right_aligned_str(
    writer: &mut dyn fmt::Write,
    amount_written: &mut usize,
    align_to: usize,
    s: &str,
) -> fmt::Result {
    let spaces_to_write = align_to
        .saturating_sub(*amount_written)
        .saturating_sub(s.len());
    if spaces_to_write > 0 {
        write!(writer, "{}", " ".repeat(spaces_to_write))?;
        *amount_written += spaces_to_write;
    }
    write!(writer, "{s}")?;
    *amount_written += s.len();
    Ok(())
}

/// Calculate a hash of the data from some part of a seekable and
/// readable file.
pub fn calc_hash_from_file_slice<SR: Seek + Read>(
    reader: &mut SR,
    offset: u64,
    size: usize,
) -> Result<u64, io::Error> {
    reader.seek(SeekFrom::Start(offset))?;

    let mut tmp = vec![0; size];
    let actual_amount = reader.read(&mut tmp)?;
    if actual_amount < size {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            format!("tried to read {size} bytes for hashing, but could only read {actual_amount}"),
        ));
    }

    let mut hasher = DefaultHasher::new();
    Hash::hash_slice(&tmp, &mut hasher);
    Ok(hasher.finish())
}

#[cfg(test)]
#[allow(clippy::unnecessary_wraps)]
mod tests {
    use super::*;

    use std::io::{Cursor, SeekFrom};

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_write_zeros_to_align_to() -> TestResult {
        for relative_to in [0, 1, 2] {
            for base in [0, 0x20, 0x40] {
                for adj in [-2, -1, 0, 1, 2] {
                    if (relative_to + base + adj) < 0 {
                        continue;
                    }
                    let written = (relative_to + base + adj) as u64;
                    let relative_to = relative_to.try_into()?;

                    if written < relative_to {
                        continue;
                    }

                    let mut cursor = Cursor::new(Vec::new());
                    if written > 0 {
                        cursor.write_all(&vec![0; written.try_into()?])?;
                    }

                    write_zeros_to_align_to(&mut cursor, 0x20, relative_to)?;

                    let final_pos = cursor.stream_position()?;

                    assert!(final_pos >= written, "cursor somehow moved backwards");
                    assert_eq!((final_pos - relative_to) % 0x20, 0, "alignment didn't work");
                }
            }
        }
        Ok(())
    }

    mod read_from_into {
        use super::*;

        #[test]
        fn test_simple() -> TestResult {
            let mut vec_1 = Cursor::new(vec![0, 1, 2, 3, 4, 5, 6, 7]);
            let mut vec_2 = Cursor::new(vec![8, 9, 10, 11, 12, 13, 14, 15]);

            vec_2.seek(SeekFrom::Start(6))?;
            read_from_into(&mut vec_1, &mut vec_2, 5)?;

            assert_eq!(vec_1.into_inner(), vec![0, 1, 2, 3, 4, 5, 6, 7]);
            assert_eq!(
                vec_2.into_inner(),
                vec![8, 9, 10, 11, 12, 13, 0, 1, 2, 3, 4]
            );

            Ok(())
        }

        #[test]
        fn test_oob() -> TestResult {
            let mut vec_1 = Cursor::new(vec![0, 1, 2, 3]);
            let mut vec_2 = Cursor::new(vec![8, 9, 10, 11]);
            assert!(read_from_into(&mut vec_1, &mut vec_2, 5).is_err());
            Ok(())
        }
    }

    #[test]
    fn test_write_right_aligned_str() -> TestResult {
        let mut s = String::new();
        let mut amount_written = 0;
        write_right_aligned_str(&mut s, &mut amount_written, 10, "")?;
        assert_eq!(s, "          ");
        assert_eq!(amount_written, 10);

        let mut s = String::new();
        let mut amount_written = 5;
        write_right_aligned_str(&mut s, &mut amount_written, 10, "aa")?;
        assert_eq!(s, "   aa");
        assert_eq!(amount_written, 10);

        let mut s = String::new();
        let mut amount_written = 7;
        write_right_aligned_str(&mut s, &mut amount_written, 10, "aa")?;
        assert_eq!(s, " aa");
        assert_eq!(amount_written, 10);

        let mut s = String::new();
        let mut amount_written = 7;
        write_right_aligned_str(&mut s, &mut amount_written, 10, "aaa")?;
        assert_eq!(s, "aaa");
        assert_eq!(amount_written, 10);

        let mut s = String::new();
        let mut amount_written = 7;
        write_right_aligned_str(&mut s, &mut amount_written, 10, "aaaa")?;
        assert_eq!(s, "aaaa");
        assert_eq!(amount_written, 11);

        Ok(())
    }

    #[test]
    fn test_calc_hash_from_file_slice() -> TestResult {
        let data = b"AAAABBBBAAAABBBBCCCCDDDD";
        let mut cursor = Cursor::new(data.to_vec());

        assert_eq!(
            calc_hash_from_file_slice(&mut cursor, 0, 8)?,
            calc_hash_from_file_slice(&mut cursor, 8, 8)?
        );
        assert_ne!(
            calc_hash_from_file_slice(&mut cursor, 0, 8)?,
            calc_hash_from_file_slice(&mut cursor, 16, 8)?
        );
        Ok(())
    }
}
