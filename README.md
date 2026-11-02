# deptree

A CLI tool that reads output from `mvn dependency:tree` (from stdin, a file, or by running `mvn` itself), builds an in-memory representation of the dependency tree, and lets you search for a package to see the whole branch it lives on — or dump the entire tree.

Direct dependencies are shown in **green**, transitive dependencies in **yellow**, and the root (the project itself) in bold.

## Installation

### Pre-built binaries

Download the latest release from [Releases](../../releases). Packages are available for:

- **Linux (x86_64):** `.tar.gz` (plain binary) or `.AppImage` (portable, runs directly)
- **macOS (universal, x86_64 + arm64):** `.tar.gz`
- **Windows (x86_64):** `.zip`

The binaries are not signed/notarized, so the OS will block them on first run unless you clear the download marker manually:

**macOS** — remove the quarantine attribute (`xattr`) after extracting, otherwise Gatekeeper will say the app "cannot be opened because the developer cannot be verified":

```sh
tar -xzf deptree-*-macos-universal.tar.gz
xattr -d com.apple.quarantine deptree-*-macos-universal/deptree
```

**Windows** — remove the "Mark of the Web" flag (the NTFS equivalent of `xattr`) after extracting, otherwise SmartScreen will block it from running:

```powershell
Expand-Archive deptree-*-x86_64-windows.zip
Unblock-File .\deptree-*-x86_64-windows\deptree.exe
```

(Alternatively: right-click the `.exe` → Properties → check "Unblock" → OK.)

**Linux** — the AppImage file needs to be made executable first:

```sh
chmod +x deptree-*-x86_64.AppImage
./deptree-*-x86_64.AppImage guava
```

If running the AppImage fails due to missing FUSE (common in containers/older distros), run it with `--appimage-extract-and-run`:

```sh
./deptree-*-x86_64.AppImage --appimage-extract-and-run guava
```

### Build from source

```sh
cargo build --release
```

The binary is placed at `target/release/deptree`. Install it locally to `~/.cargo/bin` with:

```sh
just install
# or: cargo install --path .
```

## Usage

### Find where a package lives

```sh
mvn dependency:tree | deptree guava
```

or let `deptree` run `mvn` itself (happens automatically when stdin isn't piped and no `--file` is given):

```sh
deptree guava
```

This searches for packages matching `guava` as a substring (case-insensitive) in `groupId:artifactId:packaging[:classifier]:version[:scope]`, and shows the path from the root down to each match:

```
com.example:myapp:jar:1.0.0
  com.google.guava:guava-parent:pom:31.1:compile
    com.google.guava:guava:jar:31.1:compile
```

Multiple matches are separated by a blank line.

### Show the whole tree

Omit the package name to dump the entire dependency tree:

```sh
deptree < tree.txt
```

### Read from a file

```sh
deptree --file tree.txt guava
```

## Flags

| Flag | Meaning |
|---|---|
| `-f, --file <FILE>` | Read `dependency:tree` output from a file instead of stdin/mvn |
| `--mvn-arg <ARG>` | Extra argument passed to `mvn dependency:tree` (can be given multiple times, e.g. `--mvn-arg -pl --mvn-arg mymodule`) |
| `--no-color` | Disable colors (also automatically respects the standard `NO_COLOR` environment variable) |
| `--include-scope <SCOPE>` | Show only dependencies with this scope. Can be given multiple times. The root is always shown |
| `--exclude-scope <SCOPE>` | Exclude dependencies with this scope, and their whole subtree. Can be given multiple times |
| `--no-transient` | Direct dependencies only — shorthand for `--search-depth 1` in search mode, `--depth 1` in dump mode |
| `--search-depth <N>` | **Search mode:** how deep in the tree a match can be to count as a hit (0 = root only, 1 = direct dependencies, etc.) |
| `--depth <N>` | **Dump mode:** how many levels of the tree are shown. **Search mode:** how many levels of each hit's path (from the root) are shown — identical truncated paths across multiple hits are shown only once |
| `--ignore <PACKAGE>` | Exclude hits where the package is a (transitive) dependency of `<PACKAGE>`. Can be given multiple times |
| `--module <NAME>` | In a multi-module project: show only the module(s) (root coordinates) matching this name. Can be given multiple times |
| `--json` | Print the result as JSON instead of colored text |

Valid values for `--include-scope`/`--exclude-scope` are `compile`, `provided`, `runtime`, `test`, and `system` (case-insensitive). An unknown scope prints an error and exits before any input is read:

```sh
$ deptree --exclude-scope bogus
Ukjent scope "bogus". Gyldige scopes: compile, provided, runtime, test, system
```

### Search depth vs. display depth

`--search-depth` and `--depth` solve two different problems in search mode:

- `--search-depth` decides **which** nodes count as a hit. A package name found deep in the tree, but deeper than `--search-depth`, doesn't count as a hit.
- `--depth` decides **how much** of each hit's path is shown. The path is truncated to the top N levels from the root, and if multiple hits end up with an identical truncated path, it's shown only once.

Example: searching for `tomcat` finds three hits in a deep tree (the tomcat starter itself and two of its own dependencies that also happen to have "tomcat" in their name). With `--depth 1`, only the (truncated, deduplicated) path down to the first level is shown, instead of three nearly identical blocks:

```sh
deptree --depth 1 tomcat
```

### JSON output

**Search mode** (`deptree --json guava`):

```json
{
  "matches": [
    [
      { "depth": 0, "coordinate": "com.example:myapp:jar:1.0.0", "kind": "root" },
      { "depth": 1, "coordinate": "com.google.guava:guava:jar:31.1:compile", "kind": "direct" }
    ]
  ]
}
```

**Dump mode** (`deptree --json`):

```json
{
  "tree": [
    {
      "coordinate": "com.example:myapp:jar:1.0.0",
      "kind": "root",
      "children": [ ... ]
    }
  ]
}
```

## Development

```sh
just build     # cargo build
just test      # cargo test
just lint      # cargo clippy -D warnings
just fmt       # cargo fmt
just check     # fmt-check + lint + test
```

### Cutting a release

```sh
just release 0.2.0
```

This runs `release.sh`, which:

1. Verifies the working tree is clean, you're on `main`, the tag doesn't already exist, and `CHANGELOG.md` has an `[Unreleased]` section.
2. Bumps the version in `Cargo.toml` and refreshes `Cargo.lock`.
3. Turns the `[Unreleased]` section in `CHANGELOG.md` into a dated `[0.2.0]` section.
4. Runs `just check`.
5. Commits, tags `v0.2.0`, and (after confirmation) pushes — which triggers the [release workflow](.github/workflows/release.yml) to build and publish the binaries.
