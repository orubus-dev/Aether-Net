use crate::naming::{canonicalize_name, NameError};
use crate::storage::ContentId;
use super::hosting::{DeploymentSource, SiteId, SiteManifest};
use sha2::{Digest, Sha256};

pub const MAX_SITE_FILES: usize = 65_536;
pub const MAX_FILE_SIZE: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiteFile { pub path:String, pub content:Vec<u8> }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildArtifact { pub content:ContentId, pub size:u64, pub files:usize }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeploymentPlan { pub site:SiteId, pub domain:String, pub artifact:BuildArtifact, pub rollback_to:Option<ContentId> }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeploymentError { InvalidDomain(NameError), EmptyFiles, TooManyFiles, FileTooLarge, InvalidPath, BuildMismatch }

pub fn prepare_upload(
    domain:&str,
    files:&[SiteFile],
    entrypoint:impl Into<String>,
    previous:Option<ContentId>,
)->Result<DeploymentPlan,DeploymentError>{
    let canonical=canonicalize_name(domain).map_err(DeploymentError::InvalidDomain)?;
    if files.is_empty(){return Err(DeploymentError::EmptyFiles)}
    if files.len()>MAX_SITE_FILES{return Err(DeploymentError::TooManyFiles)}
    let mut sorted=files.to_vec();
    sorted.sort_by(|a,b|a.path.as_bytes().cmp(b.path.as_bytes()));
    let mut h=Sha256::new();h.update(b"aether:site-artifact:v1:");
    for file in &sorted{
        if file.path.is_empty()||file.path.starts_with('/')||file.path.contains("..")||file.path.contains('\\')||file.content.len()>MAX_FILE_SIZE{return Err(if file.content.len()>MAX_FILE_SIZE{DeploymentError::FileTooLarge}else{DeploymentError::InvalidPath})}
        h.update((file.path.len() as u32).to_be_bytes());h.update(file.path.as_bytes());h.update((file.content.len() as u64).to_be_bytes());h.update(&file.content);
    }
    let content=ContentId::from_bytes(&h.finalize());
    let manifest=SiteManifest::new(canonical.clone(),DeploymentSource::Upload,*content.as_bytes(),entrypoint).map_err(|_|DeploymentError::InvalidPath)?;
    Ok(DeploymentPlan{site:manifest.site_id(),domain:canonical,artifact:BuildArtifact{content,size:sorted.iter().map(|f|f.content.len() as u64).sum(),files:sorted.len()},rollback_to:previous})
}

pub fn verify_artifact(plan:&DeploymentPlan, files:&[SiteFile])->Result<(),DeploymentError>{
    let rebuilt=prepare_upload(&plan.domain,files,"index.html",plan.rollback_to).map_err(|_|DeploymentError::BuildMismatch)?;
    if rebuilt.artifact.content!=plan.artifact.content{return Err(DeploymentError::BuildMismatch)} Ok(())
}

#[cfg(test)]
mod tests{
use super::*;
fn f(path:&str,b:u8)->SiteFile{SiteFile{path:path.into(),content:vec![b;4]}}
#[test]fn deterministic_artifact(){let fs=[f("index.html",1),f("assets/a.css",2)];let a=prepare_upload("Example.Ae",&fs,"index.html",None).unwrap();let b=prepare_upload("example.ae",&fs,"index.html",None).unwrap();assert_eq!(a.artifact.content,b.artifact.content);}
#[test]fn rejects_traversal(){let fs=[f("../secret",1)];assert!(matches!(prepare_upload("site.ae",&fs,"index.html",None),Err(DeploymentError::InvalidPath)));}
#[test]fn verifies_same_bytes(){let fs=[f("index.html",7)];let p=prepare_upload("site.ae",&fs,"index.html",None).unwrap();assert!(verify_artifact(&p,&fs).is_ok());}
}
