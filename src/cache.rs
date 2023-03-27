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
