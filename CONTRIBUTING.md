# Contributing to Restrict Language

Thank you for your interest in contributing to Restrict Language! We welcome contributions from the community.

## Getting Started

1. Fork the repository
2. Clone your fork: `git clone https://github.com/YOUR_USERNAME/restrict_lang.git`
3. Create a new branch: `git checkout -b feature/your-feature-name`
4. Set up your development environment:
   ```bash
   # Install mise (build tool)
   curl https://mise.jdx.dev/install.sh | sh
   
   # Install dependencies
   mise install
   mise run setup
   ```

## Development Process

### Building the Project
```bash
mise run build
```

### Running Tests
```bash
mise run test
```

### Code Style
- Follow Rust standard formatting: `cargo fmt`
- Ensure no clippy warnings: `cargo clippy`
- Run `mise run lint` before committing

## Pull Request Process

1. Ensure all tests pass
2. Update documentation if needed
3. Add tests for new features
4. Update CHANGELOG.md with your changes
5. Submit a pull request with a clear description

### Commit Message Guidelines

- Use present tense ("Add feature" not "Added feature")
- Use imperative mood ("Move cursor to..." not "Moves cursor to...")
- Limit first line to 72 characters
- Reference issues and pull requests when relevant

Example:
```
feat: Add lambda type inference

- Implement bidirectional type checking for lambdas
- Add closure capture analysis
- Update parser to support new syntax

Fixes #123
```

## Types of Contributions

### Bug Reports
- Use the issue template
- Include minimal reproduction steps
- Include version information

### Feature Requests
- Check existing issues first
- Clearly describe the use case
- Consider implementation complexity

### Code Contributions
- Small, focused pull requests are preferred
- Include tests for new functionality
- Update documentation as needed

### Documentation
- Fix typos and clarify unclear sections
- Add examples for complex features
- Translate documentation to other languages

## Code of Conduct

### Our Standards
- Be respectful and inclusive
- Welcome newcomers and help them get started
- Focus on constructive criticism
- Respect differing viewpoints

### Unacceptable Behavior
- Harassment or discrimination
- Personal attacks
- Trolling or inflammatory comments
- Publishing private information

## Development Guidelines

### Testing
- Write unit tests for new functions
- Add integration tests for new features
- Ensure all existing tests pass
- Aim for good test coverage

### Documentation
- Document public APIs with doc comments
- Update README.md for user-facing changes
- Add examples for complex features
- Keep CHANGELOG.md up to date

### Performance
- Profile before optimizing
- Document performance-critical code
- Consider WASM output size
- Avoid unnecessary allocations

## Release Process

1. Update version in Cargo.toml
2. Update CHANGELOG.md
3. Create a pull request
4. After merge, tag the release
5. GitHub Actions will handle the rest

## Questions?

Feel free to:
- Open an issue for questions
- Join our discussions
- Read the documentation at docs/

Thank you for contributing to Restrict Language!