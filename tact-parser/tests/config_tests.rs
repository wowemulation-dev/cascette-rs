//! Integration tests for configuration file parsing

use tact_parser::config::{ConfigFile, BuildConfig, CdnConfig};

#[test]
fn test_real_world_build_config() {
    // Real-world build config format based on TACT.Net and actual NGDP data
    let config_text = r#"# Build Configuration

## seqn = 3136135
root = 70c8ce1f7cf81302bc0341211b499809 31519
install = 79e1afb713f96ca3e9f049aca3f1b433 8192
install-size = 79e1afb713f96ca3e9f049aca3f1b433 8192
download = fb5ba2d2eef871e31d28c73e5d883754 16827
download-size = fb5ba2d2eef871e31d28c73e5d883754 16827
size = d8fbe632f4a0cf1d95ad2e663c32c1f1 56788
size-size = d8fbe632f4a0cf1d95ad2e663c32c1f1 56788
encoding = 9e3f7e6dc5e526ad88d14332fecb6a12 891234 0a3f7e6dc5e526ad88d14332fecb6a13 891235
encoding-size = 9e3f7e6dc5e526ad88d14332fecb6a12 891234 0a3f7e6dc5e526ad88d14332fecb6a13 891235
patch = 
patch-size = 
patch-config = 
build-name = 1.15.7.61582
build-uid = wow_classic_era
build-product = WoW
build-playbuild-installer = ngdp:us:wow_classic_era
build-partial-priority = 
vfs-root = manifest.dat 0
vfs-1 = interface.dat 0
vfs-2 = 
vfs-3 = 
    "#;
    
    let config = BuildConfig::parse(config_text).unwrap();
    
    // Test hash lookups
    assert_eq!(config.root_hash(), Some("70c8ce1f7cf81302bc0341211b499809"));
    assert_eq!(config.encoding_hash(), Some("9e3f7e6dc5e526ad88d14332fecb6a12"));
    assert_eq!(config.install_hash(), Some("79e1afb713f96ca3e9f049aca3f1b433"));
    assert_eq!(config.download_hash(), Some("fb5ba2d2eef871e31d28c73e5d883754"));
    assert_eq!(config.size_hash(), Some("d8fbe632f4a0cf1d95ad2e663c32c1f1"));
    
    // Test build info
    assert_eq!(config.build_name(), Some("1.15.7.61582"));
    assert_eq!(config.config.get_value("build-uid"), Some("wow_classic_era"));
    assert_eq!(config.config.get_value("build-product"), Some("WoW"));
    
    // Test size lookups
    assert_eq!(config.config.get_size("root"), Some(31519));
    assert_eq!(config.config.get_size("encoding"), Some(891234));
    
    // Test VFS entries
    assert_eq!(config.config.get_value("vfs-root"), Some("manifest.dat 0"));
    assert_eq!(config.config.get_value("vfs-1"), Some("interface.dat 0"));
    
    // Test empty values
    assert_eq!(config.config.get_value("patch"), Some(""));
    assert_eq!(config.config.get_hash("patch"), None);
}

#[test]
fn test_real_world_cdn_config() {
    let config_text = r#"# CDN Configuration

## seqn = 3136135
archives = 00802ffe94f0bb8e6ee6057a5e84f03c 018767e62d1ba1e1d63c693deb2e771f 01cec8eb8fc8e5dd17c22eb882b690f0
archives-index-size = 123456 234567 345678
archive-group = fb3c60af492e4bc4863e323d087e7166
patch-archives = 5782994e87743275c737f5e8d519cd1f 60bebc8d29bb2f6c4fb37bbfa440e36f
patch-archives-index-size = 456789 567890
file-index = eb439ef75c96c973c0c711117b76e61f 17024
file-index-size = eb439ef75c96c973c0c711117b76e61f 17024
patch-file-index = 1de5736c18db6e6bb3496fe635876dc8 2376
patch-file-index-size = 1de5736c18db6e6bb3496fe635876dc8 2376
    "#;
    
    let config = CdnConfig::parse(config_text).unwrap();
    
    // Test archives list
    let archives = config.archives();
    assert_eq!(archives.len(), 3);
    assert_eq!(archives[0], "00802ffe94f0bb8e6ee6057a5e84f03c");
    assert_eq!(archives[1], "018767e62d1ba1e1d63c693deb2e771f");
    assert_eq!(archives[2], "01cec8eb8fc8e5dd17c22eb882b690f0");
    
    // Test archive group
    assert_eq!(config.archive_group(), Some("fb3c60af492e4bc4863e323d087e7166"));
    
    // Test file index
    assert_eq!(config.file_index(), Some("eb439ef75c96c973c0c711117b76e61f 17024"));
    
    // Test patch archives
    let patch_archives_value = config.config.get_value("patch-archives").unwrap();
    let patch_archives: Vec<&str> = patch_archives_value.split_whitespace().collect();
    assert_eq!(patch_archives.len(), 2);
    assert_eq!(patch_archives[0], "5782994e87743275c737f5e8d519cd1f");
}

#[test]
fn test_mixed_value_types() {
    let config_text = r#"# Mixed types of values

simple-key = simple-value
number-key = 12345
hash-key = abc123def456 789012
multi-hash = abc123 100 def456 200 789012 300
quoted-value = "value with spaces"
path-value = C:\Path\To\File.dat
url-value = http://example.com/path
empty-value = 
    "#;
    
    let config = ConfigFile::parse(config_text).unwrap();
    
    // Simple values
    assert_eq!(config.get_value("simple-key"), Some("simple-value"));
    assert_eq!(config.get_value("number-key"), Some("12345"));
    
    // Hash detection
    assert_eq!(config.get_hash("hash-key"), Some("abc123def456"));
    assert_eq!(config.get_size("hash-key"), Some(789012));
    
    // Multi-hash only gets first pair
    assert_eq!(config.get_hash("multi-hash"), Some("abc123"));
    assert_eq!(config.get_size("multi-hash"), Some(100));
    
    // Complex values preserved as-is
    assert_eq!(config.get_value("quoted-value"), Some("\"value with spaces\""));
    assert_eq!(config.get_value("path-value"), Some("C:\\Path\\To\\File.dat"));
    assert_eq!(config.get_value("url-value"), Some("http://example.com/path"));
    
    // Empty value
    assert_eq!(config.get_value("empty-value"), Some(""));
}

#[test]
fn test_case_sensitive_keys() {
    let config_text = r#"
key = value1
Key = value2
KEY = value3
    "#;
    
    let config = ConfigFile::parse(config_text).unwrap();
    
    // Keys should be case-sensitive
    assert_eq!(config.get_value("key"), Some("value1"));
    assert_eq!(config.get_value("Key"), Some("value2"));
    assert_eq!(config.get_value("KEY"), Some("value3"));
    assert_eq!(config.get_value("kEy"), None);
}

#[test]
fn test_special_characters_in_values() {
    let config_text = r#"
key1 = value=with=equals
key2 = value with multiple   spaces
key3 = value	with	tabs
key4 = !@#$%^&*()_+-=[]{}|;':",./<>?
key5 = 中文字符
    "#;
    
    let config = ConfigFile::parse(config_text).unwrap();
    
    assert_eq!(config.get_value("key1"), Some("value=with=equals"));
    assert_eq!(config.get_value("key2"), Some("value with multiple   spaces"));
    assert_eq!(config.get_value("key3"), Some("value	with	tabs"));
    assert_eq!(config.get_value("key4"), Some("!@#$%^&*()_+-=[]{}|;':\",./<>?"));
    assert_eq!(config.get_value("key5"), Some("中文字符"));
}

#[test]
fn test_config_keys_method() {
    let config_text = r#"
zebra = last
apple = first
middle = center
    "#;
    
    let config = ConfigFile::parse(config_text).unwrap();
    let keys = config.keys();
    
    // Should have all keys
    assert_eq!(keys.len(), 3);
    assert!(keys.contains(&"zebra"));
    assert!(keys.contains(&"apple"));
    assert!(keys.contains(&"middle"));
}

#[test]
fn test_edge_cases() {
    // Test various edge cases
    let config_text = r#"
# Leading/trailing spaces
  spaced-key  =  spaced-value  
# Multiple equals signs
multi-equals = value = with = many = equals
# No spaces around equals (should not parse)
nospace=value
# Extra spaces around equals
extra   =   spaces
# Just key, no value
#no-value = 
# Hash that's too short (less than 6 chars)
short = abc 123
# Hash with invalid characters
invalid-hash = ghijkl 456
    "#;
    
    let config = ConfigFile::parse(config_text).unwrap();
    
    // Trimmed keys and values
    assert_eq!(config.get_value("spaced-key"), Some("spaced-value"));
    
    // Multiple equals - only first is separator
    assert_eq!(config.get_value("multi-equals"), Some("value = with = many = equals"));
    
    // No spaces around = means it won't parse
    assert_eq!(config.get_value("nospace"), None);
    
    // Extra spaces work
    assert_eq!(config.get_value("extra"), Some("spaces"));
    
    // Short hash not detected as hash
    assert_eq!(config.get_hash("short"), None);
    assert_eq!(config.get_value("short"), Some("abc 123"));
    
    // Invalid hex not detected as hash
    assert_eq!(config.get_hash("invalid-hash"), None);
    assert_eq!(config.get_value("invalid-hash"), Some("ghijkl 456"));
}

#[test]
fn test_hash_pair_struct() {
    let config_text = r#"
test-hash = fedcba9876543210 123456
    "#;
    
    let config = ConfigFile::parse(config_text).unwrap();
    let hash_pair = config.get_hash_pair("test-hash").unwrap();
    
    assert_eq!(hash_pair.hash, "fedcba9876543210");
    assert_eq!(hash_pair.size, 123456);
}