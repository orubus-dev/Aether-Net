use super::{PhysicalError, PhysicalLink};
use crate::physical::adapter::PhysicalAdapter;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicalSnapshot {
    pub adapter_count: usize,
    pub active_link_count: usize,
}

pub trait PhysicalAdapterExt {
    fn discover(&self) -> Result<Vec<PhysicalLink>, PhysicalError>;
}

pub struct PhysicalEngine {
    max_adapters: usize,
    max_links: usize,
    adapters: Vec<PhysicalAdapter>,
    links: Vec<PhysicalLink>,
}

impl PhysicalEngine {
    pub fn new(max_adapters: usize, max_links: usize) -> Self {
        Self {
            max_adapters: max_adapters.max(1),
            max_links: max_links.max(1),
            adapters: Vec::new(),
            links: Vec::new(),
        }
    }

    pub fn add_adapter(&mut self, adapter: PhysicalAdapter) -> Result<(), PhysicalError> {
        if self.adapters.len() >= self.max_adapters {
            return Err(PhysicalError::CapacityExceeded);
        }
        self.adapters.push(adapter);
        Ok(())
    }

    pub fn adapter_count(&self) -> usize {
        self.adapters.len()
    }
    pub fn links(&self) -> &[PhysicalLink] {
        &self.links
    }

    pub fn replace_links(&mut self, discovered: Vec<PhysicalLink>) -> Result<usize, PhysicalError> {
        if discovered.len() > self.max_links {
            return Err(PhysicalError::CapacityExceeded);
        }
        let mut unique = Vec::with_capacity(discovered.len());
        for link in discovered {
            validate_link(link)?;
            if let Some(index) = unique.iter().position(|existing| existing.id == link.id) {
                unique[index] = link;
            } else {
                unique.push(link);
            }
        }
        self.links = unique;
        Ok(self.links.len())
    }

    pub fn snapshot(&self) -> PhysicalSnapshot {
        PhysicalSnapshot {
            adapter_count: self.adapters.len(),
            active_link_count: self.links.iter().filter(|link| link.active).count(),
        }
    }

    pub fn discover_all(&mut self) -> Result<usize, PhysicalError> {
        let mut discovered = Vec::new();
        for adapter in &self.adapters {
            let links = adapter.discover()?;
            discovered.extend(links);
        }
        self.replace_links(discovered)
    }
}

fn validate_link(link: PhysicalLink) -> Result<(), PhysicalError> {
    if link.mtu == 0 {
        return Err(PhysicalError::InvalidMtu);
    }
    Ok(())
}
