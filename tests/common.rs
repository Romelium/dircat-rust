// tests/common.rs

use assert_cmd::prelude::*;
use std::process::Command;
// Removed unused imports: dircat::core_types::{FileInfo, FileCounts}, std::path::PathBuf

// Helper function to get the binary command
pub fn dircat_cmd() -> Command {
    Command::cargo_bin("dircat").expect("Failed to find dircat binary")
}

// Removed the unused helper function: create_mock_file_info_internal

// Potential future helpers for setting up temporary directories/files
/*
use tempfile::{tempdir, TempDir};
use std::fs;
use std::io::Write;
use std::path::Path;

pub struct TestEnv {
    pub dir: TempDir,
    pub cmd: Command,
}

pub fn setup_test_environment() -> Result<TestEnv, Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let mut cmd = dircat_cmd();
    cmd.current_dir(temp.path());
    Ok(TestEnv { dir: temp, cmd })
}

pub fn create_file(dir_path: &Path, relative_path: &str, content: &str) -> Result<(), Box<dyn std::error::Error>> {
    let file_path = dir_path.join(relative_path);
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&file_path, content)?;
    Ok(())
}
*/
