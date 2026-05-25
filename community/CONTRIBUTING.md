# Contributing to HSK Platform

Thank you for your interest in contributing to the HSK Platform! This document provides guidelines for contributing.

## Code of Conduct

This project adheres to a code of conduct. By participating, you are expected to uphold this code:

- Be respectful and inclusive
- Welcome newcomers
- Focus on constructive feedback
- Respect different viewpoints

## How to Contribute

### Reporting Bugs

Before creating a bug report:

1. Check if the issue already exists
2. Collect information about the bug
3. Prepare steps to reproduce

Bug report template:

```markdown
**Description**
Clear description of the bug

**Steps to Reproduce**
1. Go to '...'
2. Click on '...'
3. See error

**Expected Behavior**
What you expected to happen

**Actual Behavior**
What actually happened

**Environment**
- OS: [e.g. macOS 13.0]
- Browser: [e.g. Chrome 120]
- Version: [e.g. 1.0.0]
```

### Suggesting Enhancements

Enhancement suggestions should:

- Use a clear title
- Provide detailed description
- Explain the use case
- Consider implementation approach

### Pull Requests

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes
4. Run tests (`make test`)
5. Commit your changes (`git commit -m 'Add amazing feature'`)
6. Push to the branch (`git push origin feature/amazing-feature`)
7. Open a Pull Request

#### PR Checklist

- [ ] Tests added/updated
- [ ] Documentation updated
- [ ] CHANGELOG.md updated
- [ ] Code follows style guidelines
- [ ] Commit messages are clear
- [ ] No merge conflicts

### Development Setup

```bash
# Clone the repository
git clone https://github.com/hskernel/hs-verifier.git
cd hs-verifier

# Install dependencies
make install-deps

# Run tests
make test

# Start development environment
make dev-up
```

## Coding Standards

### Rust

- Follow [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `rustfmt` for formatting
- Use `clippy` for linting
- Document public APIs with rustdoc

```bash
# Format code
cargo fmt

# Run linter
cargo clippy --all-targets --all-features

# Run tests
cargo test
```

### TypeScript/JavaScript

- Use ESLint and Prettier
- Follow Airbnb style guide
- Use TypeScript for type safety

### Commit Messages

Use conventional commits:

```
<type>(<scope>): <subject>

<body>

<footer>
```

Types:
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation
- `style`: Formatting
- `refactor`: Code refactoring
- `test`: Tests
- `chore`: Maintenance

Example:
```
feat(consent): add batch consent revocation

Add ability to revoke multiple consents in a single request.
This improves UX for users with many active consents.

Closes #123
```

## Testing

### Unit Tests

```bash
cargo test --lib
```

### Integration Tests

```bash
make test-integration
```

### End-to-End Tests

```bash
make k8s-test
```

## Documentation

- Update README.md if needed
- Add rustdoc comments for public APIs
- Update architecture diagrams
- Add ADRs for significant decisions

## Security

### Reporting Security Issues

**Do not** open public issues for security vulnerabilities.

Instead, email security@hskernel.io with:
- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if any)

We will respond within 48 hours.

### Security Best Practices

- Never commit secrets
- Use environment variables for configuration
- Follow OWASP guidelines
- Run security scans: `make security-scan`

## Areas for Contribution

### High Priority

- [ ] Additional language SDKs
- [ ] Performance optimizations
- [ ] Additional verifiers
- [ ] Documentation improvements

### Medium Priority

- [ ] UI/UX improvements
- [ ] Additional integrations
- [ ] Testing improvements
- [ ] Developer tools

### Good First Issues

Look for issues labeled:
- `good-first-issue`
- `help-wanted`
- `documentation`

## Community

### Communication Channels

- GitHub Discussions: General discussion
- Discord: [Join here](https://discord.gg/hskernel)
- Twitter: [@hskplatform](https://twitter.com/hskplatform)
- Email: community@hskernel.io

### Meetings

- Weekly community call: Thursdays 10am PT
- Monthly architecture review: First Monday of month

### Recognition

Contributors will be:
- Listed in CONTRIBUTORS.md
- Mentioned in release notes
- Eligible for swag program

## License

By contributing, you agree that your contributions will be licensed under the MIT OR Apache-2.0 license.

## Questions?

- Open a GitHub Discussion
- Join our Discord
- Email: community@hskernel.io

Thank you for contributing to HSK Platform!
