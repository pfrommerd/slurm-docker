use object_store::path::Path as ObjectPath;
use object_store::ObjectStore;
use sha2::{Digest, Sha256};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

pub type Checksum = String;

#[derive(Clone, Debug)]
pub struct Layer {
    parent: Option<Arc<Layer>>,
    store: Arc<dyn ObjectStore>,
    path: ObjectPath,
    // The hash of the content stored at the specified path.
    content_hash: Checksum,
}

impl Layer {
    pub async fn checksum(
        parent: Option<Arc<Layer>>,
        store: Arc<dyn ObjectStore>,
        path: Option<&ObjectPath>,
    ) -> Self {
        // Fetch the hash of the stored content.
        let mut hasher = Sha256::new();
        for subpath in store.list(path).await? {
            let content = store.get(subpath.into()).await?;
            hasher.update(subpath);
            hasher.update(content);
        }
        let content_hash = hex::encode(hasher.finalize());
        Self {
            parent,
            store,
            path: path.into(),
            content_hash: "".into(),
        }
    }

    pub fn parent(&self) -> Option<&Arc<Layer>> {
        self.parent.as_ref()
    }

    pub fn store(&self) -> &Arc<dyn ObjectStore> {
        &self.store
    }

    pub fn path(&self) -> &ObjectPath {
        &self.path
    }

    pub fn checksum(&self) -> &Checksum {
        &self.checksum
    }
}

impl PartialEq for Layer {
    fn eq(&self, other: &Self) -> bool {
        self.parent == other.parent && self.checksum == other.checksum
    }
}

impl Eq for Layer {}

impl Hash for Layer {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.parent.hash(state);
        self.checksum.hash(state);
    }
}
