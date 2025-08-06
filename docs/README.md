# Cascette Documentation

Welcome to the Cascette documentation! This directory contains detailed technical
documentation for understanding and working with Blizzard's NGDP (Next Generation
Distribution Pipeline) protocols and formats.

## 📚 Documentation Overview

### Core Protocol Documentation

#### [BPSV Format Specification](bpsv-format.md)

The Blizzard Pipe-Separated Values (BPSV) format is the foundation of NGDP data
exchange. This document covers:

- Complete format specification with field types (STRING, HEX, DEC)
- Schema definitions and validation rules
- Sequence number handling for version tracking
- Real-world examples from Ribbit and TACT responses
- Best practices for parsing and building BPSV documents

#### [Ribbit Protocol](ribbit-protocol.md)

Ribbit is Blizzard's TCP-based protocol for retrieving version information and
metadata. This document includes:

- Protocol versions (V1 MIME-based and V2 raw PSV)
- Complete endpoint reference (Summary, Versions, CDNs, BGDL, Certs, OCSP)
- Authentication and signature verification using PKCS#7/CMS
- Region-specific server information
- Implementation notes and gotchas

#### [TACT Protocol](tact-protocol.md)

TACT (Trusted Application Content Transfer) handles content distribution from CDN
servers. This document covers:

- HTTP (v1) and HTTPS (v2) protocol versions - **v2 is now default and recommended**
- Version server endpoints and response formats
- CDN content URL structure and hash-based paths
- File formats (manifests, configurations, archives)
- BLTE encoding and encryption details
- Integration with CASC for local storage

### Additional Resources

#### [Performance Optimization Report](performance-optimization-report.md)

Detailed analysis of performance improvements implemented across all crates:

- Zero-copy parsing optimizations
- Parallel download strategies
- Streaming I/O operations
- Benchmark results and metrics

#### Temporary Research Notes (temp/)

The `temp/` directory contains research notes and analysis from studying reference
implementations. These are working documents that helped inform our implementation
decisions.

## 🔗 Quick Links

### External References

- [NGDP Overview on wowdev.wiki](https://wowdev.wiki/NGDP)
- [TACT Details on wowdev.wiki](https://wowdev.wiki/TACT)
- [CASC Storage Format](https://wowdev.wiki/CASC)
- [Wago Tools API Documentation](https://wago.tools/apis)

### Related Crate Documentation

- [ngdp-bpsv](../ngdp-bpsv/README.md) - BPSV parser/writer implementation
- [ribbit-client](../ribbit-client/README.md) - Ribbit protocol client
- [tact-client](../tact-client/README.md) - TACT HTTP/HTTPS client
- [tact-parser](../tact-parser/README.md) - TACT file format parsers
- [ngdp-cdn](../ngdp-cdn/README.md) - CDN content delivery
- [ngdp-cache](../ngdp-cache/README.md) - Caching layer
- [blte](../blte/README.md) - BLTE decompression library
- [ngdp-crypto](../ngdp-crypto/README.md) - Encryption/decryption support
- [ngdp-client](../ngdp-client/README.md) - CLI tool

## 📖 Reading Order

If you're new to NGDP, we recommend reading the documentation in this order:

1. **[BPSV Format](bpsv-format.md)** - Understanding the data format used throughout
   NGDP
2. **[Ribbit Protocol](ribbit-protocol.md)** - How to retrieve version and configuration
   information
3. **[TACT Protocol](tact-protocol.md)** - How content is distributed and downloaded

## 🎯 Use Cases

### For Library Users

- Start with the BPSV format to understand data structures
- Review the protocol documentation for the specific client you're using
- Check the performance optimization report for efficiency tips

### For Contributors

- Read all core protocol documentation thoroughly
- Review the research notes in `temp/` for implementation insights
- Understand the performance characteristics documented in the optimization report

### For WoW Emulation Developers

- Focus on the Ribbit protocol for version management
- Understand TACT for content distribution
- Review BPSV format for parsing game metadata

## 📝 Documentation Standards

All documentation in this directory follows these principles:

- Technical accuracy with references to official sources
- Clear examples demonstrating real-world usage
- Structured format with consistent headings
- Focus on implementation details relevant to Rust development

## 🔄 Keeping Documentation Updated

When making changes to the codebase:

1. Update relevant protocol documentation if behavior changes
2. Add new examples when implementing features
3. Document any discovered quirks or edge cases
4. Keep external reference links current

## 📞 Getting Help

If you have questions about the documentation:

1. Check the [main project README](../README.md) for quick start guides
2. Review the examples in each crate's `examples/` directory
3. Open an issue on GitHub for clarification requests
4. Contribute improvements via pull requests

---

**Note**: This project is not affiliated with or endorsed by Blizzard Entertainment.
It is an independent implementation based on reverse engineering efforts by the
community for educational and preservation purposes.
