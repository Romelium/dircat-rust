# dircat-rust ⚡

**Fast, `gitignore`-aware directory concatenation with Markdown output.**

[![CI Status](https://img.shields.io/github/actions/workflow/status/romelium/dircat-rust/ci.yml?branch=main&style=flat-square&logo=githubactions&logoColor=white)](https://github.com/romelium/dircat-rust/actions/workflows/ci.yml)
[![Latest Release](https://img.shields.io/github/v/release/romelium/dircat-rust?style=flat-square&logo=github&logoColor=white)](https://github.com/romelium/dircat-rust/releases/latest)
[![Crates.io](https://img.shields.io/crates/v/dircat?style=flat-square&logo=rust&logoColor=white)](https://crates.io/crates/dircat)
[![License: MIT](https://img.shields.io/crates/l/dircat?style=flat-square)](https://opensource.org/licenses/MIT)
[![Rust Version](https://img.shields.io/badge/rust-1.84.1%2B-orange?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![Downloads](https://img.shields.io/crates/d/dircat?style=flat-square)](https://crates.io/crates/dircat)

`dircat-rust` recursively walks through a directory, concatenates the content of discovered files (respecting `.gitignore` rules and offering extensive filtering), and outputs everything as a single, well-formatted Markdown file.

It's designed for speed, developer convenience, and seamless integration with tools that consume Markdown (like LLMs or documentation systems).

---

**Table of Contents**

- [dircat-rust ⚡](#dircat-rust-)
  - [Why Use `dircat-rust`?](#why-use-dircat-rust)
    - [Philosophy](#philosophy)
    - [Markdown Output Benefits](#markdown-output-benefits)
  - [Key Features](#key-features)
    - [Intelligent File Discovery](#intelligent-file-discovery)
    - [Flexible Filtering](#flexible-filtering)
    - [Content Processing](#content-processing)
    - [Customizable Output](#customizable-output)
    - [Performance](#performance)
    - [User Experience](#user-experience)
  - [Installation](#installation)
    - [Pre-compiled Binaries (Recommended)](#pre-compiled-binaries-recommended)
    - [Via Cargo](#via-cargo)
    - [From Source](#from-source)
  - [Quick Start](#quick-start)
  - [Usage](#usage)
    - [Command-Line Options](#command-line-options)
      - [Input Options](#input-options)
      - [Filtering Options](#filtering-options)
      - [Content Processing Options](#content-processing-options)
      - [Output Formatting Options](#output-formatting-options)
      - [Output Destination \& Summary Options](#output-destination--summary-options)
      - [Processing Order Options](#processing-order-options)
      - [Execution Control Options](#execution-control-options)
  - [Examples / Use Cases](#examples--use-cases)
      - [Goal: Concatenate all Rust files in `src` and `tests`](#goal-concatenate-all-rust-files-in-src-and-tests)
      - [Goal: Concatenate specific config files only](#goal-concatenate-specific-config-files-only)
      - [Goal: Copy Python code (no comments/empty lines) to clipboard](#goal-copy-python-code-no-commentsempty-lines-to-clipboard)
      - [Goal: Pipe output to `glow` for terminal rendering](#goal-pipe-output-to-glow-for-terminal-rendering)
      - [Goal: Include binary files (e.g., images) in the output](#goal-include-binary-files-eg-images-in-the-output)
      - [Goal: Exclude lockfiles from the output](#goal-exclude-lockfiles-from-the-output)
      - [Goal: Concatenate a remote git repository](#goal-concatenate-a-remote-git-repository)
  - [Tips \& Considerations](#tips--considerations)
  - [Comparison with Alternatives](#comparison-with-alternatives)
  - [Development Status \& Standards](#development-status--standards)
  - [Contributing](#contributing)
  - [License](#license)

---

## Why Use `dircat-rust`?

Are you tired of:

- Manually `cat`-ing multiple files to create context for LLMs or documentation?
- Wrestling with complex `find ... -exec` commands just to view relevant code?
- Sharing code snippets that lack structure or ignore your project's `.gitignore` rules?
- Needing a quick, readable snapshot of a directory's textual content?

`dircat-rust` solves these problems by providing a fast, configurable, and developer-friendly way to concatenate directory contents into a clean Markdown format.

### Philosophy

- **Markdown First:** Outputting Markdown provides a universally readable, portable, and easily parsable format suitable for humans, documentation systems, and AI tools.
- **Developer Focus:** Deep integration with `.gitignore` rules (via the excellent `ignore` crate) ensures the output accurately reflects the relevant parts of a typical software project. Sensible defaults like skipping binary files and an option to skip lockfiles enhance usability.
- **Performance:** Built in Rust with parallel processing (via `rayon`) to handle large directories efficiently without unnecessary overhead.

### Markdown Output Benefits

- **Readability:** Standardized, human-readable format with clear file separation and code block syntax highlighting (in compatible viewers).
- **Portability:** Easily shared and renders consistently across platforms and tools (GitHub, VS Code preview, Obsidian, etc.).
- **LLM/AI Friendly:** An excellent format for providing structured code context to Large Language Models.
- **Integration:** Can be easily included in other Markdown documents or processed by Markdown-aware tools (like static site generators or documentation tools).

## Key Features

### Intelligent File Discovery

- **Git Repository Cloning:** Process remote git repositories directly by providing a URL. `dircat` will clone the repo into a temporary location and process it.
  - **Private Repos:** Automatically uses your SSH agent or default SSH keys for authentication.
  - **Branch Selection:** Clone a specific branch with `--git-branch`.
  - **Shallow Clone:** Perform a shallow clone with `--git-depth` to save time and data.
- **Recursive Traversal:** Walks through directories recursively by default (`-n` to disable).
- **Comprehensive `.gitignore` Support:** Natively respects rules from `.gitignore`, `.ignore`, global git config files, and parent directories using the `ignore` crate. (`-t` to disable).
- **Custom Ignore Patterns:** Specify additional glob patterns to ignore files or directories (`-i`).
- **Binary File Skipping:** Skips files detected as binary/non-text by default (`--include-binary` to override).
- **Lockfile Skipping:** Option to easily skip common lockfiles (`--no-lockfiles`).

### Flexible Filtering

- **By Size:** Limit processing to files below a maximum size (`-m`, e.g., `1M`, `512k`).
- **By Extension:** Include (`-e`) or exclude (`-x`) files based on their extensions (case-insensitive).
- **By Path Regex:** Include only files whose full path matches a regular expression (`-r`).
- **By Filename Regex:** Include only files whose filename (basename) matches a regular expression (`-d`).

### Content Processing

- **Comment Removal:** Option to strip C/C++ style comments (`//`, `/* ... */`) while respecting strings (`-c`).
- **Empty Line Removal:** Option to remove lines containing only whitespace (`-l`).

### Customizable Output

- **Markdown Format:** Outputs content wrapped in Markdown code fences with language hints based on file extensions.
- **File Headers:** Clear `## File:` headers separate content from different files.
- **Filename Only Header:** Option to show only the filename in headers instead of the relative path (`-f`).
- **Line Numbers:** Prepend line numbers to each line of file content (`-L`).
- **Backticks:** Wrap filenames in headers and summaries with backticks (`-b`).
- **Summary:** Append a list of processed files, optionally with line, character, and word counts (`-s`, `-C`).

### Performance

- **Rust Speed:** Built in Rust for high performance and memory safety.
- **Parallel Processing:** Leverages Rayon for parallel file discovery and processing, speeding up operations on multi-core systems.
- **Efficient Libraries:** Uses optimized libraries like `walkdir` and `ignore` for file system operations.

### User Experience

- **Cross-Platform:** Provides pre-compiled binaries for Linux, macOS, and Windows.
- **Clone Progress:** Displays a progress bar when cloning git repositories.
- **Multiple Output Options:** Write to stdout (default), a file (`-o`), or the system clipboard (`-p`).
- **Dry Run:** Preview which files *would* be processed without reading or concatenating content (`-D`).
- **User-Friendly Errors:** Clear error messages for issues like invalid paths, incorrect arguments, or file access problems.

## Installation

### Pre-compiled Binaries (Recommended)

Download the appropriate binary for your system from the [Latest Release](https://github.com/romelium/dircat-rust/releases/latest) page.

*(Note: Binaries are self-contained and do not require installing Rust or any other language runtime.)*

**Linux (x86_64 / aarch64) / macOS (Intel x86_64 / Apple Silicon arm64):**

```bash
# --- Adjust VERSION and TARGET ---
VERSION="v0.1.0" # Replace with the desired version
# TARGET options: x86_64-unknown-linux-gnu, aarch64-unknown-linux-gnu, x86_64-apple-darwin, aarch64-apple-darwin
TARGET="x86_64-unknown-linux-gnu"
# --- --- --- --- --- --- --- ---

# Download (using curl)
curl -L "https://github.com/romelium/dircat-rust/releases/download/${VERSION}/dircat-${VERSION}-${TARGET}.tar.gz" -o dircat.tar.gz

# OR Download (using wget)
# wget "https://github.com/romelium/dircat-rust/releases/download/${VERSION}/dircat-${VERSION}-${TARGET}.tar.gz" -O dircat.tar.gz

# Extract
tar xzf dircat.tar.gz

# Make executable
chmod +x dircat

# Optional: Move to a directory in your PATH
# sudo mv dircat /usr/local/bin/
# OR (if you have a ~/bin directory in your PATH)
# mkdir -p ~/bin && mv dircat ~/bin/
```

**Windows (x86_64):**

```powershell
# --- Adjust VERSION ---
$VERSION = "v0.1.0" # Replace with the desired version
$TARGET = "x86_64-pc-windows-msvc"
# --- --- --- --- ---

# Download (using Invoke-WebRequest)
$URL = "https://github.com/romelium/dircat-rust/releases/download/${VERSION}/dircat-${VERSION}-${TARGET}.zip"
$OUTPUT = "dircat.zip"
Invoke-WebRequest -Uri $URL -OutFile $OUTPUT

# OR Download (using curl, if available)
# curl.exe -L $URL -o $OUTPUT

# Extract
Expand-Archive -Path $OUTPUT -DestinationPath .

# Optional: Add the directory containing dircat.exe to your system's PATH environment variable
# Or move dircat.exe to a directory already in your PATH
```

### Via Cargo

If you have the Rust toolchain installed (`rustup`), you can install `dircat-rust` using `cargo`:

```bash
cargo install dircat
```

*(Requires Rust 1.70 or later - check project's `Cargo.toml` for exact MSRV if specified).*

### From Source

```bash
# Clone the repository
git clone https://github.com/romelium/dircat-rust.git
cd dircat-rust

# Build the release binary
cargo build --release

# The executable will be in ./target/release/dircat
./target/release/dircat --version
```

## Quick Start

1. **Install** `dircat` using one of the methods above (pre-compiled binary recommended).
2. **Run it** in your project directory:

    ```bash
    # Concatenate all relevant text files in the current directory into output.md
    # (skips binaries, respects .gitignore by default)
    dircat . > output.md
    ```

3. **Check `output.md`!** You should see something like:

    ````markdown
    ## File: src/main.rs
    ```rs
    fn main() { /* ... */ }
    ```

    ## File: README.md
    ```md
    # My Project
    ...
    ```
    ````

🚀 **Start using `dircat` now!** Try `dircat .` in your project.

## Usage

```text
dircat [OPTIONS] [INPUT]
```

- `INPUT`: The directory, specific file, or git repository URL to process. Defaults to the current directory (`.`).

**Basic Examples:**

```bash
# Process the current directory (text files, respecting .gitignore), print to stdout
dircat

# Process the 'src' subdirectory
dircat src

# Process only a single file (binary check still applies unless --include-binary)
dircat src/main.rs

# Process a remote git repository by cloning it to a temporary directory
dircat https://github.com/romelium/dircat-rust.git

# Process a specific branch of a remote repository
dircat https://github.com/some/repo.git --git-branch develop

# Process the current directory and save to a file
dircat . -o project_snapshot.md

# Process the current directory, including binary files
dircat . --include-binary > output_with_binaries.md

# Process the current directory, excluding common lockfiles
dircat . --no-lockfiles > output_without_locks.md
```

### Command-Line Options

Below are the most common options. For a full, definitive list, run `dircat --help`.

#### Input Options

| Option                | Description                                                              |
| :-------------------- | :----------------------------------------------------------------------- |
| `[INPUT]`             | Path to a directory/file, or a git URL. Defaults to `.`.                 |
| `--git-branch BRANCH` | For git URL inputs, clone a specific branch instead of the default.      |
| `--git-depth DEPTH`   | For git URL inputs, perform a shallow clone with a limited history depth. |

#### Filtering Options

| Option             | Alias | Description                                                                                             | Example                     |
| :----------------- | :---- | :------------------------------------------------------------------------------------------------------ | :-------------------------- |
| `--max-size BYTES` | `-m`  | Skip files larger than this size (e.g., "1M", "512k", "1024").                                           | `-m 1M`                     |
| `--no-recursive`   | `-n`  | Process only the top-level directory or specified file (disable recursion).                             | `-n`                        |
| `--ext EXT`        | `-e`  | Include *only* files with these extensions (case-insensitive, repeatable).                              | `-e rs -e toml`             |
| `--exclude-ext EXT`| `-x`  | Exclude files with these extensions (case-insensitive, repeatable, overrides `-e`).                       | `-x log -x tmp`             |
| `--ignore GLOB`    | `-i`  | Ignore files/directories matching these custom glob patterns (relative to input path, repeatable).      | `-i "target/*" -i "*.lock"` |
| `--regex REGEX`    | `-r`  | Include *only* files whose full path matches any of these regexes (case-insensitive, repeatable).       | `-r "src/.*\.rs$"`          |
| `--filename-regex REGEX` | `-d` | Include *only* files whose filename matches any of these regexes (case-insensitive, repeatable). | `-d "^test_.*"`             |
| `--no-gitignore`   | `-t`  | Process all files, ignoring `.gitignore`, `.ignore`, hidden files, etc.                                 | `-t`                        |
| `--include-binary` | `-B`   | Include files detected as binary/non-text (default is to skip them).                                    | `--include-binary`          |
| `--no-lockfiles`   | `-K`   | Skip common lockfiles (e.g., `Cargo.lock`, `package-lock.json`).                                        | `--no-lockfiles`            |

#### Content Processing Options

| Option              | Alias | Description                                                     |
| :------------------ | :---- | :-------------------------------------------------------------- |
| `--remove-comments` | `-c`  | Remove C/C++ style comments (`//`, `/* ... */`) from content. |
| `--remove-empty-lines` | `-l` | Remove lines containing only whitespace from content.         |

#### Output Formatting Options

| Option             | Alias | Description                                                                       |
| :----------------- | :---- | :-------------------------------------------------------------------------------- |
| `--filename-only`  | `-f`  | Show only the filename (basename) in `## File:` headers, not the relative path. |
| `--line-numbers`   | `-L`  | Add line numbers to the beginning of each content line.                |
| `--backticks`      | `-b`  | Wrap filenames in headers and summary list with backticks (`).                    |

#### Output Destination & Summary Options

| Option        | Alias | Description                                                                 |
| :------------ | :---- | :-------------------------------------------------------------------------- |
| `--output FILE` | `-o`  | Write output to the specified file instead of stdout.                     |
| `--paste`     | `-p`  | Copy output to the system clipboard.                                        |
| `--summary`   | `-s`  | Print a summary list of processed files at the end.                       |
| `--counts`    | `-C`   | Include line, character (byte), and word counts in the summary (implies `-s`). |

#### Processing Order Options

| Option        | Alias | Description                                                                                             | Example                  |
| :------------ | :---- | :------------------------------------------------------------------------------------------------------ | :----------------------- |
| `--last GLOB` | `-z`  | Process files matching these glob patterns (relative path/filename) last, in the order specified. Repeatable. | `-z README.md -z src/main.rs` |
| `--only-last` | `-Z`  | Only process files specified with `-z`/`--last`. Skip all others (requires `-z`).                       | `-Z`                     |

#### Execution Control Options

| Option     | Alias | Description                                                                            |
| :--------- | :---- | :------------------------------------------------------------------------------------- |
| `--dry-run`| `-D`  | Print files that *would* be processed (respecting filters/order), but not content. |

💡 **Explore further!** Experiment with different filters or check `dircat --help` for all options.

## Examples / Use Cases

#### Goal: Concatenate all Rust files in `src` and `tests`

```bash
dircat . -e rs -r "^(src|tests)/" > rust_code.md
```

*Output Snippet:*

````markdown
    ## File: src/lib.rs
    ```rs
    // Library code...
    ```

    ## File: tests/integration.rs
    ```rs
        // Test code...
    ```
    ````

    #### Goal: Create context for an LLM, excluding tests, logs, and comments

    ```bash
        dircat . -e rs -e py -e toml -x log -i "tests/*" -c --no-lockfiles -o llm_context.md
    ```

    *Output Snippet:*

    ````markdown
    ## File: src/config.py
    ```py
        # Config loading logic (comments removed)
    ```

    ## File: Cargo.toml
    ```toml
        # Dependencies (comments removed)
    ```
    ````

    #### Goal: See which files would be included if max size is 50kB

    ```bash
        dircat . -m 50k -D
    ```

    *Output Snippet:*

    ```
        --- Dry Run: Files that would be processed ---
        - src/small_module.rs
        - config/settings.toml
        --- End Dry Run ---
    ```

    #### Goal: Process `README.md` and `LICENSE` last

    ```bash
        dircat . -z README.md -z LICENSE > project_with_readme_last.md
    ```

    *Output Snippet:* (Other files appear first, then README, then LICENSE)

    ````markdown
    ...
    ## File: src/main.rs
    ```rs
        ...
    ```
    ...
    ## File: README.md
    ```md
        ...
    ```

    ## File: LICENSE
    ```
        ...
    ```
````

#### Goal: Concatenate specific config files only

```bash
dircat . -z "config/*.toml" -z ".env.example" -Z > config_files.md
```

*Output Snippet:* (Only files matching the `-z` patterns are included)

````markdown
    ## File: config/database.toml
    ```toml
        ...
    ```

    ## File: .env.example
    ```
        VAR=value
    ```
````

#### Goal: Copy Python code (no comments/empty lines) to clipboard

```bash
dircat src -e py -c -l -p
```

#### Goal: Pipe output to `glow` for terminal rendering

```bash
dircat src -e rs | glow -
```

#### Goal: Include binary files (e.g., images) in the output

```bash
dircat assets --include-binary > assets_output.md
```

#### Goal: Exclude lockfiles from the output

```bash
dircat . --no-lockfiles > project_without_locks.md
```

#### Goal: Concatenate a remote git repository

```bash
# Clones the repo to a temporary directory and processes it.
# Automatically uses SSH keys for private repos.
# Displays a progress bar for long clones.
dircat git@github.com:romelium/dircat-rust.git > repo_content.md

# Clone a specific branch
dircat https://github.com/some/repo.git --git-branch develop

# Perform a shallow clone of depth 1
dircat https://github.com/some/repo.git --git-depth 1
```

## Tips & Considerations

- **Large Output:** Running `dircat` on large directories can produce significant output. Consider using filters (`-m`, `-e`, `-r`, etc.) or the dry-run (`-D`) option first. Use `-o FILE` to redirect large outputs to a file instead of overwhelming your terminal.
- **Binary Files:** By default, `dircat` attempts to skip binary files. Use `-B` if you need to include them (e.g., for embedding small images represented as text, though this is generally not recommended for large binaries). The detection is heuristic and might not be perfect.
- **Lockfiles:** Use `-K` to easily exclude common dependency lockfiles. This is often desirable when generating context for LLMs.
- **Path Handling:**
  - **Display:** File paths shown in `## File:` headers and the summary (`-s`) are relative to the *input path* you provided (or the current directory if none was given).
  - **Filtering:**
    - Path Regex (`-r`): Matches against the full path, normalized to use `/` separators.
    - Filename Regex (`-d`): Matches against the filename (basename) only.
    - Ignore/Last Globs (`-i`, `-z`): Match against the path relative to the *input path*.
- **Performance:** While `dircat-rust` is designed for speed, processing extremely large files or a vast number of files will still take time. Use filters to narrow down the scope when possible.

## Comparison with Alternatives

| Feature                 | `dircat-rust`          | `cat`          | `find ... -exec cat {} +` | `tree`         |
| :---------------------- | :--------------------- | :------------- | :------------------------ | :------------- |
| **Directory Input**     | ✅ Yes                 | ❌ No          | ✅ Yes (via `find`)       | ✅ Yes         |
| **Concatenate Content** | ✅ Yes                 | ✅ Yes (files) | ✅ Yes                    | ❌ No          |
| **Gitignore Aware**     | ✅ Yes (Built-in)      | ❌ No          | Manual (complex)          | Manual         |
| **Markdown Output**     | ✅ Yes                 | ❌ No          | ❌ No                     | ❌ No          |
| **Skip Binaries**       | ✅ Yes (Default)       | Reads all      | Manual (e.g., `file`)     | N/A            |
| **Skip Lockfiles**      | ✅ Yes (`-K`)| ❌ No          | Manual (`-name`)          | Manual         |
| **Filtering (Size)**    | ✅ Yes (`-m`)          | ❌ No          | Manual (`-size`)          | Manual         |
| **Filtering (Ext/Regex)**| ✅ Yes (`-e`/`-x`/`-r`/`-d`) | ❌ No          | Manual (`-name`/`-regex`) | Manual         |
| **Content Processing**  | ✅ Yes (`-c`/`-l`)      | ❌ No          | Manual (e.g., `sed`)      | ❌ No          |
| **Speed Focus**         | ✅ Yes (Rust/Parallel) | Fast (single)  | Variable                  | Fast (metadata)|
| **Cross-Platform Binaries** | ✅ Yes             | OS specific    | OS specific               | OS specific    |

## Development Status & Standards

`dircat-rust` is under active development. It utilizes modern Rust practices, including:

- Continuous Integration (CI) via GitHub Actions.
- Code formatting (`cargo fmt`) and linting (`cargo clippy`).
- Conventional Commits for clear commit history.
- Pre-commit hooks to enforce standards before committing.

## Contributing

Contributions are welcome! Whether it's bug reports, feature suggestions, or code improvements, please feel free to:

1. Check the [Issue Tracker](https://github.com/romelium/dircat-rust/issues) for existing bugs or ideas.
2. Open a new issue to discuss your suggestion or report a bug.
3. Review the [Commit Message Guidelines](COMMIT.md) before submitting pull requests.
4. Set up pre-commit hooks locally (`pre-commit install`) to ensure your contributions meet project standards.

🤝 **We welcome contributions!** Please see our `COMMIT.md` guidelines and check the issue tracker for ways to help.

## License

This project is licensed under the **MIT License**. See the [LICENSE](LICENSE) file for details.
