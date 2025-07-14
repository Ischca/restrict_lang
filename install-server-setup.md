# Install Server Setup

To make `curl -sSf https://install.restrict-lang.org | sh` work, we need:

## 1. Domain Setup

- Register `restrict-lang.org` domain
- Point `install.restrict-lang.org` to hosting

## 2. Simple Solution: GitHub Pages

Use raw GitHub content:

```bash
# Users can use this right now:
curl -sSf https://raw.githubusercontent.com/restrict-lang/restrict_lang/main/scripts/install.sh | sh
```

## 3. Better Solution: CDN

### Using GitHub Pages

1. Create `restrict-lang/restrict-lang.github.io` repository
2. Add CNAME file with `install.restrict-lang.org`
3. Copy install script to `index.html` or `install.sh`

### Using Cloudflare Pages

1. Connect GitHub repository
2. Deploy scripts/install.sh
3. Set up custom domain

## 4. Current Working Solution

Update documentation to use:

```bash
# This works today!
curl -sSf https://raw.githubusercontent.com/restrict-lang/restrict_lang/main/scripts/install.sh | sh

# Or with wget
wget -qO- https://raw.githubusercontent.com/restrict-lang/restrict_lang/main/scripts/install.sh | sh
```

## 5. Security Considerations

Add integrity check:

```bash
# Download and verify
curl -sSf https://raw.githubusercontent.com/restrict-lang/restrict_lang/main/scripts/install.sh -o install.sh
echo "EXPECTED_SHA256  install.sh" | sha256sum -c
sh install.sh
```