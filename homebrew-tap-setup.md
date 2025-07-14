# Homebrew Tap Setup Guide

## 1. Create GitHub Repository

Create a new repository named `homebrew-tap` under the restrict-lang organization:
- Repository name: `restrict-lang/homebrew-tap`
- Public repository
- Initialize with README

## 2. Repository Structure

```
homebrew-tap/
├── Formula/
│   └── warder.rb
├── README.md
└── .github/
    └── workflows/
        └── test.yml
```

## 3. Move Formula

Move the formula we created to the tap repository:

```bash
# Clone the tap repository
git clone https://github.com/restrict-lang/homebrew-tap.git
cd homebrew-tap

# Create Formula directory
mkdir -p Formula

# Copy the formula
cp ../restrict_lang/homebrew/warder.rb Formula/

# Update formula with actual SHA256
# After creating a release, update the sha256 field
```

## 4. Update Formula for Real Release

```ruby
class Warder < Formula
  desc "Package manager for Restrict Language"
  homepage "https://restrict-lang.org"
  version "0.1.0"
  
  # For binary releases (recommended)
  if OS.mac? && Hardware::CPU.intel?
    url "https://github.com/restrict-lang/restrict_lang/releases/download/v0.1.0/restrict-lang-v0.1.0-darwin-x86_64.tar.gz"
    sha256 "ACTUAL_SHA256_HERE"
  elsif OS.mac? && Hardware::CPU.arm?
    url "https://github.com/restrict-lang/restrict_lang/releases/download/v0.1.0/restrict-lang-v0.1.0-darwin-aarch64.tar.gz"
    sha256 "ACTUAL_SHA256_HERE"
  elsif OS.linux?
    url "https://github.com/restrict-lang/restrict_lang/releases/download/v0.1.0/restrict-lang-v0.1.0-linux-x86_64.tar.gz"
    sha256 "ACTUAL_SHA256_HERE"
  end

  def install
    bin.install "restrict_lang"
    bin.install "warder"
  end

  test do
    assert_match "warder", shell_output("#{bin}/warder --version")
  end
end
```

## 5. Test Formula Locally

```bash
# Test the formula
brew install --build-from-source Formula/warder.rb

# Or for debugging
brew install --verbose --debug Formula/warder.rb
```

## 6. Add CI Testing

Create `.github/workflows/test.yml`:

```yaml
name: Test Formula

on:
  push:
    branches: [main]
  pull_request:

jobs:
  test:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Test formula
        run: |
          brew install --build-from-source Formula/warder.rb
          brew test warder
```

## 7. Publish the Tap

```bash
# Commit and push
git add .
git commit -m "Add warder formula"
git push origin main
```

## 8. Users Can Now Install

Once published, users can install with:

```bash
brew tap restrict-lang/tap
brew install warder
```

## 9. Updating the Formula

When releasing new versions:

1. Update version and URLs in formula
2. Calculate new SHA256:
   ```bash
   curl -L https://github.com/restrict-lang/restrict_lang/releases/download/v0.2.0/restrict-lang-v0.2.0-darwin-x86_64.tar.gz | shasum -a 256
   ```
3. Update the formula
4. Push to tap repository

## Alternative: Homebrew Core

For inclusion in homebrew-core (more strict):

1. Formula must build from source
2. Project must have a stable release
3. Project must be notable (GitHub stars, downloads, etc.)
4. No pre-built binaries allowed
5. Must pass all Homebrew standards

Submit via:
```bash
brew bump-formula-pr --url=https://github.com/restrict-lang/restrict_lang/archive/v0.1.0.tar.gz warder
```

## Current Status

- [ ] Create homebrew-tap repository
- [ ] Create first release with binaries
- [ ] Calculate SHA256 for each platform
- [ ] Update formula with real values
- [ ] Test installation
- [ ] Document in README