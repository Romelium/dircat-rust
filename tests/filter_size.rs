// tests/filter_size.rs

mod common;

use assert_cmd::prelude::*;
use common::dircat_cmd;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_max_size_bytes() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    fs::write(temp.path().join("small.txt"), "12345")?; // 5 bytes
    fs::write(temp.path().join("exact.txt"), "1234567890")?; // 10 bytes
    fs::write(temp.path().join("large.txt"), "1234567890A")?; // 11 bytes

    dircat_cmd()
        .arg("-m")
        .arg("10") // Max 10 bytes
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: small.txt"))
        .stdout(predicate::str::contains("12345"))
        .stdout(predicate::str::contains("## File: exact.txt"))
        .stdout(predicate::str::contains("1234567890"))
        .stdout(predicate::str::contains("## File: large.txt").not())
        .stdout(predicate::str::contains("1234567890A").not());

    temp.close()?;
    Ok(())
}

#[test]
fn test_max_size_kilobytes() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let content_1k = "A".repeat(1000); // Exactly 1k (using SI def)
    let content_2k = "B".repeat(2000); // Exactly 2k
    let content_slightly_more = "C".repeat(1001); // > 1k

    fs::write(temp.path().join("1k.txt"), &content_1k)?;
    fs::write(temp.path().join("2k.txt"), &content_2k)?;
    fs::write(temp.path().join("over_1k.txt"), &content_slightly_more)?;

    dircat_cmd()
        .arg("-m")
        .arg("1k") // Max 1 kilobyte (1000 bytes)
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: 1k.txt")) // Include 1k
        .stdout(predicate::str::contains(&content_1k))
        .stdout(predicate::str::contains("## File: 2k.txt").not()) // Exclude 2k
        .stdout(predicate::str::contains(&content_2k).not())
        .stdout(predicate::str::contains("## File: over_1k.txt").not()) // Exclude > 1k
        .stdout(predicate::str::contains(&content_slightly_more).not());

    temp.close()?;
    Ok(())
}

#[test]
fn test_max_size_megabytes_binary() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let content_1mib = "A".repeat(1024 * 1024); // Exactly 1 MiB
    let content_small = "B".repeat(500 * 1024); // 500 KiB

    fs::write(temp.path().join("1mib.bin"), &content_1mib)?;
    fs::write(temp.path().join("small.bin"), &content_small)?;

    dircat_cmd()
        .arg("-m")
        .arg("1MiB") // Max 1 Mebibyte (1024*1024 bytes)
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: 1mib.bin")) // Include 1MiB
        .stdout(predicate::str::contains(&content_1mib))
        .stdout(predicate::str::contains("## File: small.bin")) // Include smaller
        .stdout(predicate::str::contains(&content_small));

    // Now test excluding the 1MiB file
    dircat_cmd()
        .arg("-m")
        .arg("1000KiB") // Max 1000 KiB (1000 * 1024 = 1,024,000 bytes) - less than 1MiB
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: 1mib.bin").not()) // Exclude 1MiB
        .stdout(predicate::str::contains(&content_1mib).not())
        .stdout(predicate::str::contains("## File: small.bin")) // Include smaller
        .stdout(predicate::str::contains(&content_small));

    temp.close()?;
    Ok(())
}
