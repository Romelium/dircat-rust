// src/output/header.rs

use crate::constants;
use anyhow::Result;
use std::io::Write;

/// Writes the global header to the output.
pub(crate) fn write_global_header(writer: &mut dyn Write) -> Result<()> {
    writer.write_all(constants::OUTPUT_FILE_HEADER.as_bytes())?;
    // Add an extra newline for spacing after the header
    writer.write_all(b"\n")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::Cursor;

    #[test]
    fn test_write_global_header_output() -> Result<()> {
        let mut writer = Cursor::new(Vec::new());
        write_global_header(&mut writer)?;
        let output = String::from_utf8(writer.into_inner())?;
        let expected = "\n";
        assert_eq!(output, expected);
        Ok(())
    }
}
