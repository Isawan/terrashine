use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use url::Url;

/// Versions response from terrashine mirror registry
#[derive(Serialize)]
pub(crate) struct MirrorVersions {
    pub(crate) archives: HashMap<String, MirrorDownloadDetail>,
}

#[derive(Serialize)]
pub(crate) struct MirrorDownloadDetail {
    pub(crate) url: Url,
    pub(crate) hashes: Option<Vec<String>>,
}

#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct MirrorVersion {
    pub(crate) archives: HashMap<String, TargetPlatformIdentifier>,
}

#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct TargetPlatformIdentifier {
    pub(crate) url: String,
}

/// Index response from terrashine mirror registry
#[derive(Serialize, Debug)]
pub(crate) struct MirrorIndex {
    // TODO: the nested hash value is always empty, we should implement
    // custom serialize to avoid unneeded work.
    pub(crate) versions: HashMap<String, HashMap<String, String>>,
}
