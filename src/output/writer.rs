// src/output/writer.rs

//! Manages the output destination (stdout, file, or clipboard).
//!
//! This module provides functions to set up the appropriate `Write` trait object
//! based on the user's configuration and to handle finalization steps, such as
//! copying the output to the clipboard.

use crate::config::{Config, OutputDestination};
use anyhow::{anyhow, Result};
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::sync::{Arc, Mutex};

/// Represents the setup output writer, potentially including a buffer for clipboard.
pub struct OutputWriterSetup {
    /// A boxed `Write` trait object that can be written to.
    pub writer: Box<dyn Write + Send>,
    /// Holds the buffer only if the destination is Clipboard.
    /// This is needed to retrieve the content for copying after all writes are complete.
    pub clipboard_buffer: Option<Arc<Mutex<Vec<u8>>>>,
}

/// Sets up the output writer based on the configuration.
///
/// This function determines whether to write to stdout, a file, or an in-memory
/// buffer (for clipboard operations) and returns a struct containing the appropriate
/// writer and any necessary context.
///
/// # Arguments
/// * `config` - The application configuration specifying the output destination.
///
/// # Returns
/// An `OutputWriterSetup` struct containing the writer and an optional clipboard buffer.
///
/// # Errors
/// Returns an error if a file cannot be created for writing.
pub fn setup_output_writer(config: &Config) -> Result<OutputWriterSetup> {
    let mut clipboard_buffer = None;
    let writer: Box<dyn Write + Send> = match &config.output_destination {
        OutputDestination::Stdout => Box::new(io::stdout()),
        OutputDestination::File(path) => {
            let file =
                File::create(path).map_err(|e| crate::errors::io_error_with_path(e, path))?;
            Box::new(BufWriter::new(file)) // Use BufWriter for file I/O
        }
        OutputDestination::Clipboard => {
            // For clipboard, write to an in-memory buffer first.
            let buffer = Arc::new(Mutex::new(Vec::<u8>::new()));
            clipboard_buffer = Some(buffer.clone()); // Store handle to the buffer
            Box::new(ArcMutexVecWriter(buffer)) // Wrap Arc<Mutex<Vec<u8>>>
        }
    };
    Ok(OutputWriterSetup {
        writer,
        clipboard_buffer,
    })
}

/// Finalizes output, specifically handling the clipboard case.
///
/// If the destination was `OutputDestination::Clipboard`, this function copies the
/// content from the provided buffer to the system clipboard. For other destinations,
/// it ensures the writer is flushed.
///
/// # Arguments
/// * `writer` - The writer to be flushed and dropped.
/// * `clipboard_buffer` - The buffer containing the output, if the destination was clipboard.
/// * `config` - The application configuration.
///
/// # Errors
/// Returns an error if locking the clipboard buffer fails or if the clipboard
/// operation itself fails.
pub fn finalize_output(
    mut writer: Box<dyn Write + Send>, // Take ownership to ensure drop/flush
    clipboard_buffer: Option<Arc<Mutex<Vec<u8>>>>,
    config: &Config,
) -> Result<()> {
    // Ensure final flush before potential clipboard op or drop
    writer.flush()?;

    if config.output_destination == OutputDestination::Clipboard {
        if let Some(buffer_arc) = clipboard_buffer {
            let buffer = buffer_arc
                .lock()
                .map_err(|e| anyhow!("Failed to lock clipboard buffer mutex: {}", e))?;
            let content = String::from_utf8(buffer.clone())?; // Clone buffer data

            copy_to_clipboard(&content)?;
            // Avoid printing to stderr in library code, let the caller handle feedback
            // eprintln!("Output copied to clipboard."); // Provide feedback
        } else {
            // This indicates an internal logic error in setup/finalize pairing
            return Err(anyhow!(
                "Clipboard destination specified, but no buffer found during finalization."
            ));
        }
    }
    // For Stdout or File, flushing happened above, and drop handles closing.
    Ok(())
}

fn copy_to_clipboard(content: &str) -> Result<()> {
    use crate::errors::AppError;
    use arboard::Clipboard;
    let mut clipboard = Clipboard::new().map_err(|e| AppError::ClipboardError(e.to_string()))?;
    clipboard
        .set_text(content)
        .map_err(|e| AppError::ClipboardError(e.to_string()))?;
    Ok(())
}

// --- Wrapper struct for Arc<Mutex<Vec<u8>>> to implement Write ---
// This is necessary because we cannot implement a foreign trait (Write)
// directly on a foreign type (Arc<Mutex<Vec<u8>>>).
#[derive(Debug, Clone)]
struct ArcMutexVecWriter(Arc<Mutex<Vec<u8>>>);

impl Write for ArcMutexVecWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // Attempt to lock the mutex. If poisoned, return an error.
        let mut buffer = self
            .0 // Access the inner Arc<Mutex<Vec<u8>>>
            .lock()
            .map_err(|e| io::Error::other(format!("Mutex poisoned: {}", e)))?;
        // Write data to the underlying Vec<u8>
        buffer.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        // Attempt to lock the mutex. If poisoned, return an error.
        let mut buffer = self
            .0 // Access the inner Arc<Mutex<Vec<u8>>>
            .lock()
            .map_err(|e| io::Error::other(format!("Mutex poisoned: {}", e)))?;
        // Flush the underlying Vec<u8> (no-op, but required by trait)
        buffer.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::tests::create_mock_config; // Use shared helper
    use tempfile::NamedTempFile;

    #[test]
    fn test_write_impl_for_arc_mutex_vec() -> io::Result<()> {
        let buffer_arc: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        let mut writer = ArcMutexVecWriter(buffer_arc.clone()); // Use wrapper as writer

        write!(writer, "Hello")?;
        writer.write_all(b", ")?;
        write!(writer, "World!")?;
        writer.flush()?; // Should be a no-op for Vec but test it doesn't fail

        // Lock the original Arc to check the contents
        let buffer = buffer_arc.lock().unwrap();
        assert_eq!(*buffer, b"Hello, World!");

        Ok(())
    }

    #[test]
    fn test_setup_output_writer_stdout() {
        // Simple check: Does it return *something* without panicking for stdout?
        // Testing the actual type is brittle.
        let config = create_mock_config(false, false, false, false); // Destination is stdout
        let setup_result = setup_output_writer(&config);
        assert!(setup_result.is_ok());
        let setup = setup_result.unwrap();
        assert!(setup.clipboard_buffer.is_none());
        // Cannot easily assert the type of writer is stdout without downcasting, which is complex.
    }

    #[test]
    fn test_setup_output_writer_file() -> Result<()> {
        let temp_file = NamedTempFile::new()?;
        let path = temp_file.path().to_path_buf();
        let mut config = create_mock_config(false, false, false, false);
        config.output_destination = OutputDestination::File(path.clone());

        let setup_result = setup_output_writer(&config);
        assert!(setup_result.is_ok());
        let mut setup = setup_result.unwrap();
        assert!(setup.clipboard_buffer.is_none());

        // Test writing to the file via the writer
        write!(setup.writer, "Test content")?;
        setup.writer.flush()?; // Important for BufWriter

        // Drop the writer to close the file before reading
        drop(setup.writer);

        // Re-open and read the file to verify content
        let content = std::fs::read_to_string(&path)?;
        assert_eq!(content, "Test content");

        Ok(())
    }

    #[test]
    fn test_setup_output_writer_clipboard() {
        // Test clipboard destination setup
        let mut config = create_mock_config(false, false, false, false);
        config.output_destination = OutputDestination::Clipboard;

        let setup_result = setup_output_writer(&config);
        assert!(setup_result.is_ok());
        let setup = setup_result.unwrap();
        assert!(setup.clipboard_buffer.is_some()); // Should have a buffer

        // Check if the writer is the buffer itself by trying to write to it
        let mut writer = setup.writer;
        write!(writer, "Clipboard test").unwrap();
        writer.flush().unwrap();

        // Check the buffer content directly
        let buffer_arc = setup.clipboard_buffer.unwrap();
        let buffer = buffer_arc.lock().unwrap();
        assert_eq!(*buffer, b"Clipboard test");
    }

    #[test]
    fn test_finalize_output_stdout() -> Result<()> {
        // Finalize should be a no-op for stdout
        let config = create_mock_config(false, false, false, false); // stdout
        let writer = Box::new(io::sink()); // Use sink to avoid printing during test
        let buffer = None;
        finalize_output(writer, buffer, &config)?; // Should just succeed
        Ok(())
    }

    #[test]
    fn test_finalize_output_file() -> Result<()> {
        // Finalize should be a no-op for file (drop handles closing)
        let temp_file = NamedTempFile::new()?;
        let path = temp_file.path().to_path_buf();
        let file = File::create(&path)?;
        let writer = Box::new(BufWriter::new(file));
        let mut config = create_mock_config(false, false, false, false);
        config.output_destination = OutputDestination::File(path);
        let buffer = None;

        finalize_output(writer, buffer, &config)?; // Should just succeed
        Ok(())
    }

    // Note: Testing finalize_output for Clipboard requires mocking `copy_to_clipboard`
    // or enabling the "clipboard" feature and potentially running in a specific environment.
    // The current test only checks if it attempts to access the buffer.
    #[test]
    fn test_finalize_output_clipboard_buffer_access() {
        let mut config = create_mock_config(false, false, false, false);
        config.output_destination = OutputDestination::Clipboard;
        let buffer_arc: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(b"clipboard data".to_vec()));
        let writer = Box::new(ArcMutexVecWriter(buffer_arc.clone())); // Writer is the buffer wrapper
        let clipboard_buffer = Some(buffer_arc.clone());

        // This will call copy_to_clipboard internally.
        let result = finalize_output(writer, clipboard_buffer, &config);

        // In a test environment without a clipboard service, arboard might return an error.
        // We accept Ok or a specific ClipboardError here.
        if let Err(e) = result {
            use crate::errors::AppError; // Need AppError for matching
            assert!(e
                .downcast_ref::<AppError>()
                .is_some_and(|ae| matches!(ae, AppError::ClipboardError(_))));
        }
    }

    #[test]
    fn test_finalize_output_clipboard_missing_buffer() {
        // Test the internal error case where buffer is somehow None
        let mut config = create_mock_config(false, false, false, false);
        config.output_destination = OutputDestination::Clipboard;
        let writer = Box::new(io::sink()); // Not a buffer
        let clipboard_buffer = None; // Missing buffer

        let result = finalize_output(writer, clipboard_buffer, &config);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("no buffer found during finalization"));
    }
}
