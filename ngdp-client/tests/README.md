# ngdp-client Tests

This directory contains comprehensive tests for the `ngdp-client` CLI application and library.

## Test Files

### CLI Integration Tests
- `builds_test.rs` - Historical builds command testing
- `certs_test.rs` - Certificate operations testing  
- `cli_test.rs` - General CLI functionality testing
- `fallback_test.rs` - Ribbit/TACT fallback testing

### Unit Tests (in source files)
Located in `src/` under `#[cfg(test)]` modules

## Test Coverage

### CLI Command Testing
- ✅ Help and version commands
- ✅ Output format options (text, JSON, BPSV)
- ✅ Global flags and options
- ✅ Error message formatting
- ✅ Exit code validation

### Product Commands
- ✅ Products list with filtering
- ✅ Products versions with region handling
- ✅ Products info with detailed output
- ✅ Products CDNs configuration display
- ✅ Products builds with historical data

### Certificate Commands
- ✅ Certificate download by SKI
- ✅ PEM and DER format output
- ✅ Certificate details extraction
- ✅ JSON output format
- ✅ Error handling for missing certificates

### Inspect Commands
- ✅ BPSV data inspection and parsing
- ✅ Build configuration analysis
- ✅ Multiple output format support
- ✅ Error handling for invalid data

### Configuration Management
- ✅ Config show command
- ✅ Config get/set operations
- ✅ Config reset functionality
- ✅ Validation and error handling

### Fallback Functionality
- ✅ Automatic Ribbit to TACT fallback
- ✅ Regional mapping (SG → US)
- ✅ Error recovery and resilience
- ✅ Caching integration
- ✅ Performance comparison

## Output Format Testing

Tests verify all output formats work correctly:
- **Text format**: Beautiful terminal output with Unicode tables and colors
- **JSON format**: Structured data output for programmatic use
- **Pretty JSON**: Human-readable JSON with proper formatting  
- **BPSV format**: Raw Blizzard protocol format

## Error Handling Testing

Comprehensive error scenario coverage:
- **Network failures**: Connection timeouts, DNS issues
- **Invalid input**: Malformed commands, wrong parameters
- **Authentication issues**: Certificate problems, access denied
- **Data corruption**: Invalid responses, parsing failures
- **Rate limiting**: API quota exceeded scenarios

## Integration Testing

Tests cover integration with all dependent crates:
- **ribbit-client**: Version and CDN queries
- **tact-client**: Alternative protocol access
- **ngdp-cache**: Transparent caching functionality
- **ngdp-bpsv**: Data parsing and formatting
- **ngdp-cdn**: Content download operations

## Running Tests

```bash
# Run all ngdp-client tests
cargo test -p ngdp-client

# Run with output to see CLI interactions
cargo test -p ngdp-client -- --nocapture

# Run specific test categories
cargo test -p ngdp-client cli
cargo test -p ngdp-client builds
cargo test -p ngdp-client certs
cargo test -p ngdp-client fallback

# Run with network access for integration tests
cargo test -p ngdp-client --features network-tests

# Run performance tests
cargo test -p ngdp-client --release -- performance
```

## Test Environment

Tests handle various environments gracefully:
- **Offline mode**: Tests that don't require network access
- **Limited connectivity**: Graceful degradation for slow/unreliable connections
- **Regional restrictions**: Handling of blocked or inaccessible servers
- **Rate limiting**: Proper backoff and retry behavior

## Mock Testing

Some tests use controlled environments:
- **Mock servers** for predictable error testing
- **Synthetic data** for edge case validation  
- **Controlled responses** for format verification
- **Error injection** for resilience testing

## Performance Testing

Tests include performance validation:
- **Command startup time**: CLI initialization speed
- **Data processing speed**: Large response handling
- **Memory usage**: Resource consumption monitoring
- **Cache effectiveness**: Hit/miss ratio validation

### Performance Expectations
- Command startup: <500ms for most operations
- Data processing: >1MB/s for BPSV parsing
- Memory usage: <50MB for typical operations
- Cache hit rate: >80% for repeated operations

## Configuration Testing

Tests verify configuration handling:
- **Default values**: Proper initialization
- **Environment variables**: Override behavior  
- **Config files**: File-based configuration
- **Command-line flags**: Argument precedence
- **Validation**: Input sanitization and validation

## User Experience Testing

Tests ensure good user experience:
- **Help messages**: Comprehensive and helpful
- **Error messages**: Clear and actionable
- **Progress indicators**: Appropriate feedback
- **Color support**: Proper terminal detection
- **Accessibility**: Support for various terminal types

This comprehensive test suite ensures the CLI tool is robust, user-friendly, and performs well across different environments and use cases.