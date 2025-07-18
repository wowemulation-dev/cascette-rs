use rand::seq::SliceRandom;

/// CDN host list provider
pub trait CdnHostProvider {
    /// Get a list of CDN hosts that should be tried for a request.
    ///
    /// The selection criteria is implementation dependent.
    fn get(&self) -> Vec<&str>;
}

/// CDN host list provider that always returns the same hostname.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SingleHost(pub String);

impl CdnHostProvider for SingleHost {
    fn get(&self) -> Vec<&str> {
        vec![self.0.as_str()]
    }
}

impl From<String> for SingleHost {
    fn from(value: String) -> Self {
        Self(value)
    }
}

/// CDN host list provider that works with a simple list of hostnames, and
/// selects them at random each time.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct StaticHostList(pub Vec<String>);

impl CdnHostProvider for StaticHostList {
    fn get(&self) -> Vec<&str> {
        let mut o: Vec<&str> = self.0.iter().map(String::as_str).collect();
        let mut rng = rand::rng();
        o.shuffle(&mut rng);

        o
    }
}

impl From<Vec<String>> for StaticHostList {
    fn from(value: Vec<String>) -> Self {
        Self(value)
    }
}

/// CDN host list provider that works with multiple lists of hostnames, and
/// selects them at random each time, working from the first list to the last.
///
/// This is ideal when some CDN hosts should be considered a last resort.
///
/// # Example
///
/// ```rust
/// # use ngdp_cdn::PriorityHostList;
/// # fn example() {
/// // Example with multiple tiers of CDN hosts
/// let list = PriorityHostList(vec![
///     // Primary CDN hosts
///     vec![
///         "primary-eu.example.com".to_string(),
///         "primary-us.example.com".to_string(),
///     ],
///     // Community CDN hosts for old files
///     vec![
///         "community.example.net".to_string(),
///         "community.example.org".to_string(),
///     ],
/// ]);
/// # }
/// ```
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PriorityHostList(pub Vec<Vec<String>>);

impl CdnHostProvider for PriorityHostList {
    fn get(&self) -> Vec<&str> {
        let mut rng = rand::rng();
        self.0
            .iter()
            .map(|list| {
                let mut list: Vec<&str> = list.iter().map(String::as_str).collect();
                list.shuffle(&mut rng);
                list
            })
            .flatten()
            .collect()
    }
}
