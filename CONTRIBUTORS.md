# Contributors

Thank you to everyone who has contributed to the `cascette-rs` project!

## Project Lead

- **Daniel S. Reichenbach** ([@danielsreichenbach](https://github.com/danielsreichenbach)) - Project creator and maintainer

## Core Contributors

*This section will be updated as the project grows and receives contributions.*

## How to Contribute

We welcome contributions from the community! Here are some ways you can help:

### Code Contributions

1. **Fork the repository** and create your feature branch (`git checkout -b feature/amazing-feature`)
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
- **Test with real NGDP/TACT data**: Verify functionality with actual Blizzard CDN data

### Areas Where Help is Needed

Here are specific areas where contributions would be especially valuable:

#### 🌐 NGDP Protocol Implementation

1. **TACT Protocol Enhancement**
   - Complete TACT v2 protocol support
   - Implement missing endpoints (product configs, install manifests)
   - Add support for patch files and differential updates
   - Implement TACT file encoding/decoding
   - Add support for archive indices

2. **CASC Storage Format**
   - Implement CASC container format reader/writer
   - Support for all CASC encoding types
   - Implement local CASC storage management
   - Add CASC file extraction and verification
   - Support for CASC root and encoding files

3. **Ribbit Protocol Features**
   - Add support for all Ribbit v2 endpoints
   - Implement Ribbit command pipelining
   - Add support for custom Ribbit queries
   - Improve MIME parsing edge cases
   - Add support for compressed Ribbit responses

#### 📦 Cache System Improvements

1. **Advanced Caching Features**
   - Implement cache size limits and eviction policies
   - Add cache compression support
   - Implement distributed caching support
   - Add cache statistics and monitoring
   - Support for cache warming strategies

2. **Performance Optimizations**
   - Implement zero-copy parsing where possible
   - Add memory-mapped file support for large caches
   - Optimize cache lookup performance
   - Implement cache preloading
   - Add async cache operations

#### 🔧 Client Library Features

1. **Enhanced Error Handling**
   - Implement retry strategies for all client types
   - Add circuit breaker patterns
   - Improve error messages and diagnostics
   - Add telemetry and metrics support
   - Implement request/response interceptors

2. **Authentication & Security**
   - Implement Blizzard authentication support
   - Add certificate pinning
   - Support for proxy authentication
   - Implement request signing
   - Add support for encrypted communications

#### 🛠️ CLI Tool Enhancements

1. **Additional Commands**
   - Implement `download` command for fetching game content
   - Add `verify` command for checking file integrity
   - Implement `extract` command for CASC archives
   - Add `sync` command for keeping local content updated
   - Implement `server` command for running a local CDN mirror

2. **User Experience**
   - Add interactive mode with command completion
   - Implement progress bars for long operations
   - Add support for configuration profiles
   - Implement parallel downloads
   - Add resume support for interrupted operations

#### 📊 Data Processing

1. **File Format Support**
   - Implement build configuration parsing
   - Add CDN configuration processing
   - Support for product configuration files
   - Implement encoding/root file parsing
   - Add support for install and download manifests

2. **Content Analysis**
   - Add tools for analyzing game patches
   - Implement content diff generation
   - Add support for content verification
   - Implement manifest comparison tools
   - Add content statistics generation

#### 🧪 Testing and Quality

1. **Test Coverage**
   - Increase test coverage to >90%
   - Add property-based testing
   - Implement integration tests with mock servers
   - Add performance regression tests
   - Create end-to-end test scenarios

2. **Documentation**
   - Write comprehensive API documentation
   - Create tutorials for common use cases
   - Document the NGDP protocol details
   - Add architecture documentation
   - Create migration guides from other tools

#### 🔌 Integrations

1. **Language Bindings**
   - Create Python bindings using PyO3
   - Add C/C++ bindings
   - Implement JavaScript/WASM support
   - Create .NET bindings
   - Add Ruby bindings

2. **Tool Integration**
   - Docker image with CLI tools
   - GitHub Actions for automated downloads
   - Jenkins plugin for CI/CD
   - Kubernetes operators for content management
   - Terraform providers for infrastructure

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
4. **Start small** - documentation fixes and small features are great first contributions
5. **Join the discussion** in issues and pull requests

### Recognition

All contributors will be recognized in this file. Significant contributions may
also be highlighted in:

- Release notes
- Project README
- Documentation credits

## 📄 License

This project is dual-licensed under either:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this project by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.

## Code of Conduct

Please note that this project follows our [Code of Conduct](CODE_OF_CONDUCT.md).
By participating, you are expected to uphold this code.

## Contact

- Open an issue for questions or discussions
- For security concerns, please see [SECURITY.md](SECURITY.md)

---

*Want to see your name here? We'd love to have your contribution! Check the issues
labeled "good first issue" to get started.*
