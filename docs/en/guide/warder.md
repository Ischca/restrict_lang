# Warder Package Manager

Warder is the official package manager for Restrict Language, designed to simplify dependency management, project scaffolding, and code distribution. Named after a guardian or keeper, Warder manages your project's dependencies in a secure "vault" while providing seamless integration with the Restrict compiler.

## Key Concepts

### Cage Format (.rgc)

The Cage format is Warder's package distribution format, similar to npm's tarballs or Rust's crates. A cage contains:
- Compiled WebAssembly modules
- Source code (optional)
- Package metadata
- Dependencies

### The Vault (restrict-lock.toml)

The Vault is your project's dependency lock file, ensuring reproducible builds across different environments. It records exact versions and checksums of all dependencies.

### WardHub

WardHub is the central registry for Restrict packages, where developers can publish and discover packages.

## Installation

Warder comes bundled with the Restrict Language compiler. Verify installation:

```bash
warder --version
```

## Getting Started

### Creating a New Project

```bash
warder new my-project
cd my-project
```

This creates a new Restrict project with the following structure:

```
my-project/
├── package.rl.toml     # Package manifest
├── src/
│   └── main.rl         # Entry point
├── tests/
│   └── main_test.rl    # Example test
└── .gitignore
```

### Initializing an Existing Project

```bash
warder init
```

This creates a `package.rl.toml` file in the current directory.

## Package Manifest (package.rl.toml)

The package manifest defines your project's metadata and dependencies:

```toml
[package]
name = "my-awesome-lib"
version = "0.1.0"
authors = ["Your Name <you@example.com>"]
description = "A brief description of your package"
license = "MIT"
repository = "https://github.com/username/my-awesome-lib"
keywords = ["web", "async", "http"]

[dependencies]
# Regular dependencies
http = "0.8.0"
json = { version = "1.0", features = ["streaming"] }
utils = { git = "https://github.com/user/utils", branch = "main" }

[dev-dependencies]
# Dependencies only for development/testing
test-framework = "0.5.0"

[build-dependencies]
# Dependencies for build scripts
wasm-bindgen = "0.2"

[target.'cfg(wasm)'.dependencies]
# Platform-specific dependencies
wasm-specific = "0.1.0"

[features]
default = ["std"]
std = []
no-std = []

[[bin]]
name = "my-app"
path = "src/bin/main.rl"

[lib]
name = "my_lib"
path = "src/lib.rl"
```

## Managing Dependencies

### Adding Dependencies

Add a dependency from WardHub:

```bash
warder add http
warder add json@1.0.0
warder add async-runtime --features runtime,macros
```

Add a development dependency:

```bash
warder add --dev test-framework
```

Add from Git:

```bash
warder add --git https://github.com/user/package
```

### Updating Dependencies

Update all dependencies to their latest compatible versions:

```bash
warder update
```

Update a specific dependency:

```bash
warder update http
```

### Removing Dependencies

```bash
warder remove http
```

### Listing Dependencies

View your project's dependency tree:

```bash
warder tree
```

Output:
```
my-project v0.1.0
├── http v0.8.0
│   ├── async-io v1.3.0
│   └── url v2.2.0
├── json v1.0.0
└── utils v0.2.0 (git+https://github.com/user/utils)
```

## Building Projects

### Development Build

```bash
warder build
```

This compiles your project in debug mode with optimizations disabled for faster compilation.

### Release Build

```bash
warder build --release
```

Produces an optimized WebAssembly module ready for production.

### Building Specific Targets

```bash
warder build --target wasm32-wasi
warder build --target wasm32-unknown-unknown
```

## Running Projects

### Running the Main Binary

```bash
warder run
```

### Running a Specific Binary

```bash
warder run --bin my-app
```

### Running with Arguments

```bash
warder run -- arg1 arg2
```

## Testing

### Running Tests

```bash
warder test
```

### Running Specific Tests

```bash
warder test test_name
warder test --test integration_test
```

### Test Coverage

```bash
warder test --coverage
```

## Publishing Packages

### Preparing for Publication

1. Ensure your `package.rl.toml` is complete
2. Add a README.md file
3. Choose an appropriate license
4. Test your package thoroughly

### Publishing to WardHub

```bash
warder login
warder publish
```

Before publishing, Warder will:
- Validate the package manifest
- Run tests
- Check for common issues
- Build the package

### Versioning

Follow semantic versioning:
- MAJOR: Breaking changes
- MINOR: New features (backward compatible)
- PATCH: Bug fixes

Update version:

```bash
warder version patch  # 0.1.0 -> 0.1.1
warder version minor  # 0.1.1 -> 0.2.0
warder version major  # 0.2.0 -> 1.0.0
```

## Workspaces

For multi-package projects, use workspaces:

```toml
# workspace.rl.toml
[workspace]
members = [
    "packages/core",
    "packages/cli",
    "packages/web"
]

[workspace.dependencies]
common = { path = "packages/common" }
```

## Advanced Features

### Custom Registries

Configure alternative registries:

```toml
[registries]
my-registry = { index = "https://my-registry.com/index" }

[dependencies]
private-package = { version = "1.0", registry = "my-registry" }
```

### Vendoring Dependencies

Download all dependencies locally:

```bash
warder vendor
```

### Cage Inspection

Examine a cage file:

```bash
warder cage inspect package-1.0.0.rgc
```

Extract a cage:

```bash
warder cage extract package-1.0.0.rgc
```

### Build Scripts

Add a build script in `build.rl`:

```restrict
fn main() {
    // Generate code, compile resources, etc.
    generateBindings();
}
```

Configure in `package.rl.toml`:

```toml
[package]
build = "build.rl"
```

## Configuration

### Global Configuration

Located at `~/.warder/config.toml`:

```toml
[registry]
default = "https://wardhub.io"
token = "your-auth-token"

[build]
jobs = 4
target-dir = "target"

[net]
offline = false
timeout = 30
```

### Project Configuration

Override global settings in `.warder/config.toml`:

```toml
[build]
opt-level = 3
debug = false
```

## Troubleshooting

### Check Project Health

```bash
warder doctor
```

This command checks:
- Package manifest validity
- Dependency conflicts
- Missing files
- Configuration issues

### Clean Build Artifacts

```bash
warder clean
```

### Verbose Output

```bash
warder build --verbose
```

### Offline Mode

Work without network access:

```bash
warder build --offline
```

## Integration with CI/CD

### GitHub Actions

```yaml
name: Build and Test
on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - uses: restrict-lang/setup-restrict@v1
      - run: warder test
      - run: warder build --release
```

### Docker

```dockerfile
FROM restrict-lang/restrict:latest
WORKDIR /app
COPY . .
RUN warder build --release
CMD ["warder", "run", "--release"]
```

## Best Practices

1. **Always commit restrict-lock.toml** - Ensures reproducible builds
2. **Use semantic versioning** - Makes dependency resolution predictable
3. **Minimize dependencies** - Smaller binaries and attack surface
4. **Test before publishing** - Run `warder test` and `warder package`
5. **Document your package** - Include examples and API documentation

## Common Commands Reference

| Command | Description |
|---------|-------------|
| `warder new <name>` | Create a new project |
| `warder init` | Initialize existing project |
| `warder build` | Compile the project |
| `warder run` | Build and run |
| `warder test` | Run tests |
| `warder add <pkg>` | Add dependency |
| `warder remove <pkg>` | Remove dependency |
| `warder update` | Update dependencies |
| `warder publish` | Publish to WardHub |
| `warder search <query>` | Search packages |
| `warder doc` | Generate documentation |
| `warder clean` | Remove build artifacts |
| `warder tree` | Show dependency tree |
| `warder doctor` | Check project health |

## Summary

Warder provides a complete package management solution for Restrict Language, from project creation to deployment. Its integration with the compiler, focus on security, and WebAssembly-first approach make it ideal for modern web development.