```bash
# Create new project
warder new my-app
cd my-app

# Add a direct local dependency
warder add local_utils --path ../local-utils

# Build and run
warder build --release
warder run
```
