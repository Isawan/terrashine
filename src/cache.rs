use std::collections::hash_map::DefaultHasher;

struct ProviderVersionsKey {
    hostname: String,
    namespace: String,
    provider_type: String,
}
struct ProviderResponseKey {
    hostname: String,
    namespace: String,
    provider_type: String,
}

struct Version(u64);

type ProviderVersionsCache =
    evmap::ReadHandle<ProviderVersionsKey, crate::index::ProviderVersions, Version, DefaultHasher>;

type IndexMirrorCache = evmap::ReadHandle<
    ProviderResponseKey,
    crate::version::ProviderResponse,
    Version,
    DefaultHasher,
>;

struct Test {
    source: String,
}
