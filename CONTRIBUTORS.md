# Contributors

Thank you to everyone who has contributed to the `cascette-rs` project!

## Project Lead

- **Daniel S. Reichenbach**

  ([@danielsreichenbach](https://github.com/danielsreichenbach)) - Project
  creator and maintainer

## Core Contributors

_This section will be updated as the project grows and receives contributions._

## How to Contribute

We welcome contributions from the community! Here are some ways you can help:

### Code Contributions

1. **Fork the repository** and create your feature branch

   (`git checkout -b feature/amazing-feature`)

2. **Make your changes** following the Rust style guidelines
3. **Add tests** for any new functionality
4. **Ensure all tests pass** (`cargo test --all-features`)
5. **Run quality checks**:

   ```bash
   cargo fmt --all
   cargo check --all-features --all-targets
   cargo clippy --all-targets --all-features
   cargo test
   ```

6. **Update documentation** if you're changing public APIs
7. **Commit your changes** with descriptive commit messages
8. **Push to your branch** and open a Pull Request

### Other Ways to Contribute

- **Report bugs**: Open an issue describing the problem with reproduction steps

- **Suggest features**: Open an issue with your enhancement proposal

- **Improve documentation**: Help make our docs clearer and more comprehensive

- **Add examples**: Create examples showing different use cases

- **Performance improvements**: Profile and optimize the code

- **Test with real NGDP/TACT data**: Verify functionality with actual Blizzard

  CDN data

### Areas Where Help is Needed

Here are specific areas where contributions would be especially valuable:

#### GUI Applications (Phase 5)

- Cross-platform GUI launcher application
- Game library browser with visual content display
- Installation progress visualization
- Settings and configuration UI

#### Server Infrastructure

- Content build system for ingestion and archiving
- CDN content management tools
- Automated mirror synchronization

#### Language Bindings

- Python bindings using PyO3
- C/C++ bindings for native integration
- JavaScript/WASM support for web applications
- .NET bindings for C# ecosystem

#### Cache System Improvements

- Distributed caching support
- Cache compression
- Cache warming strategies

#### Content Analysis Tools

- Build tools using the existing format parsers
- Tools for analyzing game patches using existing PA format support
- Content diff generation using Root and Encoding file parsers
- Manifest comparison tools using Install and Download parsers

#### Testing and Quality

- Increase test coverage to >90%
- Add property-based testing for more formats
- Implement integration tests with mock servers
- Add performance regression tests
- Create end-to-end test scenarios

#### Documentation

- Create tutorials for common use cases
- Create migration guides from other tools
- Improve inline code documentation

#### Tool Integration

- Docker image with CLI tools
- GitHub Actions for automated downloads
- Kubernetes operators for content management

### Development Guidelines

- **Code Style**: Follow Rust idioms and conventions

- **Documentation**: Document public APIs with examples

- **Testing**: Write tests for new functionality

- **Performance**: Profile before optimizing

- **Compatibility**: Support all Blizzard regions and products

- **Safety**: Prefer safe Rust, document and isolate unsafe code

### Getting Started with Contributing

1. **Check existing issues** for something you'd like to work on
2. **Comment on the issue** to let others know you're working on it
3. **Ask questions** if you need clarification
4. **Start small** - documentation fixes and small features are great first

   contributions

5. **Join the discussion** in issues and pull requests

### Recognition

All contributors will be recognized in this file. Significant contributions may
also be highlighted in:

- Release notes

- Project README

- Documentation credits

## License

This project is dual-licensed under either:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))

- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this project by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions.

## Code of Conduct

This project follows community guidelines for respectful participation.
By contributing, you are expected to maintain a respectful and constructive
environment.

## Contact

- Open an issue for questions or discussions

- For security concerns, please email
[daniel@kogito.network](mailto:daniel@kogito.network)

---

_Want to see your name here? We'd love to have your contribution! Check the
issues labeled "good first issue" to get started._
