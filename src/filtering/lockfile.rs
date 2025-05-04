// src/filtering/lockfile.rs

use std::path::Path;

// List of common lockfile names (case-insensitive check)
const LOCKFILE_NAMES: &[&str] = &[
    // --- Web Development (JavaScript/TypeScript) ---
    "package-lock.json",   // npm v5+
    "yarn.lock",           // Yarn v1 (and often checked for v2+ PnP)
    "pnpm-lock.yaml",      // pnpm
    "npm-shrinkwrap.json", // Older npm, or for library publishing
    "bun.lockb",           // Bun (binary format)
    "deno.lock",           // Deno
    // --- PHP ---
    "composer.lock", // Composer
    // --- Ruby ---
    "gemfile.lock", // Bundler (Note: Capital 'G' in original, lowercase here for comparison)
    // --- Python ---
    "poetry.lock",    // Poetry
    "pipfile.lock",   // Pipenv (Note: Capital 'P' in original, lowercase here)
    "pdm.lock",       // PDM
    "uv.lock",        // uv
    "conda-lock.yml", // conda-lock tool (used with Conda)
    ".req.lock",      // A convention sometimes used with pip-tools
    // --- Go ---
    "go.sum",     // Go Modules (checksums, acts like a lock)
    "gopkg.lock", // dep (older Go package manager) (Note: Capital 'G' in original)
    "glide.lock", // Glide (older Go package manager)
    // --- Java ---
    "gradle.lockfile", // Gradle (when locking is enabled, root file)
    // Individual Gradle configuration lockfiles might exist too, e.g., `gradle/dependency-locks/compileClasspath.lockfile`
    // Maven doesn't have a standard single lockfile, relies on plugins or conventions

    // --- .NET (C#/F#/VB.NET) ---
    "packages.lock.json",  // NuGet (when locking is enabled)
    "paket.lock",          // Paket package manager
    "project.assets.json", // NuGet internal build artifact, sometimes checked in, contains resolved graph
    // --- Swift / Objective-C (Apple Ecosystem) ---
    "package.resolved",  // Swift Package Manager (SPM) (Note: Capital 'P')
    "podfile.lock",      // CocoaPods (Note: Capital 'P')
    "cartfile.resolved", // Carthage (Note: Capital 'C')
    // --- Elixir ---
    "mix.lock", // Mix
    // --- Erlang ---
    "rebar.lock", // Rebar3
    // --- Dart / Flutter ---
    "pubspec.lock", // Pub
    // --- Haskell ---
    "stack.yaml.lock",      // Stack
    "cabal.project.freeze", // Cabal (newer versions)
    // --- Rust ---
    "cargo.lock", // Cargo (Note: Capital 'C' in original, lowercase here)
    // --- Nix / NixOS ---
    "flake.lock", // Nix Flakes
    // --- Perl ---
    "cpanfile.snapshot", // Carton / cpanm --installdeps .
    "meta.json",         // Can contain resolved dependencies, though more a manifest
    "mymeta.json",       // Similar to META.json
    // --- R ---
    "renv.lock",            // renv package manager
    "packrat/packrat.lock", // packrat package manager
    // --- Julia ---
    "manifest.toml", // Julia Pkg manager (Project.toml is the manifest) (Note: Capital 'M')
    // --- Infrastructure as Code ---
    ".terraform.lock.hcl", // Terraform provider lock file
    // --- Crystal ---
    "shard.lock", // Shards package manager
    // --- C/C++ ---
    "conan.lock", // Conan package manager lockfile
    // vcpkg uses baselines in vcpkg.json, not a separate lockfile typically

    // --- Lua ---
    "luarocks.lock", // LuaRocks (if locking feature is used, might vary)
    // --- Elm ---
    "elm-stuff/exact-dependencies.json", // Elm (internal, but effectively the lock)
    // --- BuckleScript / ReScript ---
    // Often uses Node.js ecosystem tools (npm/yarn/pnpm), covered above

    // --- Bazel ---
    // Bazel often relies on WORKSPACE pinning or bzlmod lockfiles (`MODULE.bazel.lock` potentially)
    "module.bazel.lock", // Bazel bzlmod lockfile (experimental/newer) (Note: Capital 'M')

                         // Add even more niche or specific tool lockfiles if encountered
];

/// Checks if a path corresponds to a common lockfile name.
pub(crate) fn is_lockfile(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name_str| {
            let lower_name = name_str.to_lowercase();
            LOCKFILE_NAMES
                .iter()
                // *** Fix: Compare lowercase filename with lowercase lockfile name from list ***
                .any(|&lockfile| lower_name == lockfile.to_lowercase())
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_is_lockfile_matches() {
        assert!(is_lockfile(&PathBuf::from("path/to/Cargo.lock")));
        assert!(is_lockfile(&PathBuf::from("package-lock.json")));
        assert!(is_lockfile(&PathBuf::from("Yarn.lock"))); // Case insensitive
        assert!(is_lockfile(&PathBuf::from("PNPM-LOCK.YAML"))); // Case insensitive
        assert!(is_lockfile(&PathBuf::from("go.sum")));
        assert!(is_lockfile(&PathBuf::from("Gemfile.lock"))); // Check original case
        assert!(is_lockfile(&PathBuf::from("gemfile.lock"))); // Check lowercase
        assert!(is_lockfile(&PathBuf::from("Pipfile.lock"))); // Check original case
        assert!(is_lockfile(&PathBuf::from("pipfile.lock"))); // Check lowercase
        assert!(is_lockfile(&PathBuf::from("Manifest.toml"))); // Check original case
        assert!(is_lockfile(&PathBuf::from("manifest.toml"))); // Check lowercase
    }

    #[test]
    fn test_is_lockfile_no_match() {
        assert!(!is_lockfile(&PathBuf::from("src/main.rs")));
        assert!(!is_lockfile(&PathBuf::from("Cargo.toml")));
        assert!(!is_lockfile(&PathBuf::from("lockfile.txt")));
        assert!(!is_lockfile(&PathBuf::from("noextension")));
        assert!(!is_lockfile(&PathBuf::from("path/to/"))); // Directory path
    }

    #[test]
    fn test_is_lockfile_root() {
        assert!(is_lockfile(&PathBuf::from("Cargo.lock")));
    }
}
