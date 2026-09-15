use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

pub const SITE_ID_LENGTH: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeploymentSource {
    Upload,
    GitHub {
        repository: String,
        reference: String,
        subdirectory: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SiteManifest {
    pub domain: String,
    pub source: DeploymentSource,
    pub root_resource: [u8; SITE_ID_LENGTH],
    pub entrypoint: String,
}

impl SiteManifest {
    pub fn new(
        domain: impl Into<String>,
        source: DeploymentSource,
        root_resource: [u8; SITE_ID_LENGTH],
        entrypoint: impl Into<String>,
    ) -> Result<Self, HostingError> {
        let domain = domain.into();
        validate_domain(&domain)?;
        let entrypoint = entrypoint.into();
        if entrypoint.is_empty() {
            return Err(HostingError::EmptyEntrypoint);
        }
        Ok(Self {
            domain,
            source,
            root_resource,
            entrypoint,
        })
    }

    pub fn site_id(&self) -> SiteId {
        SiteId::derive(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SiteId([u8; SITE_ID_LENGTH]);

impl SiteId {
    pub fn derive(manifest: &SiteManifest) -> Self {
        let bytes = serde_json::to_vec(manifest).expect("SiteManifest is serializable");
        let mut hasher = Sha256::new();
        hasher.update(b"aether:site:v1:");
        hasher.update(bytes);
        Self(hasher.finalize().into())
    }

    pub const fn as_bytes(&self) -> &[u8; SITE_ID_LENGTH] {
        &self.0
    }

    pub fn to_hex(self) -> String {
        let mut output = String::with_capacity(SITE_ID_LENGTH * 2);
        for byte in self.0 {
            use fmt::Write as _;
            write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
        }
        output
    }
}

impl fmt::Display for SiteId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_hex())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostingError {
    InvalidDomain,
    EmptyEntrypoint,
}

fn validate_domain(domain: &str) -> Result<(), HostingError> {
    if domain.is_empty() || domain.len() > 253 || domain.starts_with('.') || domain.ends_with('.') {
        return Err(HostingError::InvalidDomain);
    }

    for label in domain.split('.') {
        if label.is_empty()
            || label.len() > 63
            || label.starts_with('-')
            || label.ends_with('-')
            || !label.chars().all(|character| {
                character.is_ascii_alphanumeric() || character == '-'
            })
        {
            return Err(HostingError::InvalidDomain);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upload_manifest_has_stable_site_id() {
        let manifest = SiteManifest::new(
            "example.ae",
            DeploymentSource::Upload,
            [4; SITE_ID_LENGTH],
            "index.html",
        )
        .unwrap();
        assert_eq!(manifest.site_id(), manifest.site_id());
    }

    #[test]
    fn github_source_is_supported() {
        let manifest = SiteManifest::new(
            "app.ae",
            DeploymentSource::GitHub {
                repository: "owner/repo".into(),
                reference: "main".into(),
                subdirectory: None,
            },
            [8; SITE_ID_LENGTH],
            "index.html",
        )
        .unwrap();
        assert_eq!(manifest.domain, "app.ae");
    }

    #[test]
    fn invalid_domains_are_rejected() {
        assert!(SiteManifest::new(
            "not a domain",
            DeploymentSource::Upload,
            [0; SITE_ID_LENGTH],
            "index.html",
        )
        .is_err());
    }
}
