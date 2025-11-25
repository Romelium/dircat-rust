#![cfg(feature = "git")]

use dircat::git::get_repo;
use std::fs;
use std::sync::{Arc, Barrier};
use std::thread;
use tempfile::tempdir;

/// Helper to create a bare git repo to act as a "remote"
fn setup_remote_repo() -> (tempfile::TempDir, String) {
    let temp_dir = tempdir().unwrap();
    let repo_path = temp_dir.path();
    let repo = git2::Repository::init_bare(repo_path).unwrap();

    // Create a commit
    let signature = git2::Signature::now("Test", "test@example.com").unwrap();
    let tree_id = {
        let mut index = repo.index().unwrap();
        let oid = repo.blob("content".as_bytes()).unwrap();
        let entry = git2::IndexEntry {
            ctime: git2::IndexTime::new(0, 0),
            mtime: git2::IndexTime::new(0, 0),
            dev: 0,
            ino: 0,
            mode: 0o100644,
            uid: 0,
            gid: 0,
            file_size: 7,
            id: oid,
            flags: 0,
            flags_extended: 0,
            path: b"file.txt".to_vec(),
        };
        index.add(&entry).unwrap();
        index.write_tree().unwrap()
    };
    let tree = repo.find_tree(tree_id).unwrap();
    repo.commit(Some("HEAD"), &signature, &signature, "Initial", &tree, &[])
        .unwrap();

    #[cfg(windows)]
    let url = format!("file:///{}", repo_path.to_str().unwrap().replace('\\', "/"));
    #[cfg(not(windows))]
    let url = format!("file://{}", repo_path.to_str().unwrap());

    (temp_dir, url)
}

#[test]
fn test_concurrent_clones_wait_for_lock() {
    let (_remote_dir, remote_url) = setup_remote_repo();
    let cache_dir = tempdir().unwrap();
    let cache_path = cache_dir.path().to_path_buf();

    let thread_count = 5;
    let barrier = Arc::new(Barrier::new(thread_count));
    let mut handles = vec![];

    for i in 0..thread_count {
        let url = remote_url.clone();
        let c_path = cache_path.clone();
        let b = barrier.clone();

        handles.push(thread::spawn(move || {
            // Wait for all threads to be ready to maximize contention
            b.wait();

            // Attempt to get the repo
            let result = get_repo(&url, &None, None, &c_path, None, None);

            assert!(
                result.is_ok(),
                "Thread {} failed to get repo: {:?}",
                i,
                result.err()
            );
            let path = result.unwrap();
            assert!(path.exists(), "Thread {} repo path does not exist", i);
            assert!(
                path.join("file.txt").exists(),
                "Thread {} repo content missing",
                i
            );
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify the cache directory structure
    // Should contain 1 directory (the repo hash) and 1 lock file
    let entries: Vec<_> = fs::read_dir(&cache_path).unwrap().collect();
    assert!(
        entries.len() >= 2,
        "Expected at least repo dir and lock file"
    );
}
