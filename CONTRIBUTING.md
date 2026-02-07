# Contributing to tunebox

Thanks for your interest in contributing!

## Getting Started

1. Fork the repo
2. Clone your fork: `git clone https://github.com/YOUR_USERNAME/tunebox.git`
3. Create a branch: `git checkout -b my-feature`
4. Make your changes
5. Test locally: `cargo run -- ~/Music`
6. Commit: `git commit -m "Add my feature"`
7. Push: `git push origin my-feature`
8. Open a Pull Request

## Development

```bash
# Run in debug mode
cargo run -- /path/to/music

# Build release
cargo build --release

# Run tests
cargo test

# Check formatting
cargo fmt --check

# Run clippy
cargo clippy
```

## Code Style

- Run `cargo fmt` before committing
- Keep functions focused and small
- Add comments for non-obvious logic
- Follow existing patterns in the codebase

## What to Contribute

- Bug fixes
- New themes
- New visualizer modes
- Performance improvements
- Documentation improvements
- Anything tasteful

## Questions?

Open an issue if you have questions or want to discuss a feature before implementing.
