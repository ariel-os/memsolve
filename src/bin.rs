use std::ops::Deref;

use crate::section::Section;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Bin {
    bins: Vec<MemoryBin>,
}

impl Bin {
    pub(crate) fn new(bins: Vec<MemoryBin>) -> Self {
        Self { bins }
    }

    pub(crate) fn num_pages(&self) -> u64 {
        self.bins.iter().fold(0, |acc, b| acc + b.num_pages())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MemoryBin {
    pub(crate) start_address: u64,
    pub(crate) end_address: u64,
    pub(crate) page_size: u64,
}

impl MemoryBin {
    pub(crate) fn num_pages(&self) -> u64 {
        (self.end_address - self.start_address) / self.page_size
    }

    pub(crate) fn space_until_end(&self, address: u64) -> u64 {
        if !self.contains(address) {
            return 0;
        }
        self.end_address.saturating_sub(address)
    }

    pub(crate) fn pages_until_end(&self, address: u64) -> u64 {
        let space = self.space_until_end(address);
        space.strict_div(self.page_size)
    }

    pub(crate) fn section_fits_at_address<MetaData: Clone>(
        &self,
        address: u64,
        section: &Section<MetaData>,
    ) -> bool {
        if let Some(page_count) = section.pages
            && page_count > self.pages_until_end(address)
        {
            false
        } else if let Some(size) = section.size
            && size > self.space_until_end(address)
        {
            false
        } else {
            true
        }
    }

    pub(crate) fn contains(&self, address: u64) -> bool {
        address < self.end_address && address >= self.start_address
    }
}

impl Deref for Bin {
    type Target = [MemoryBin];

    fn deref(&self) -> &Self::Target {
        self.bins.deref()
    }
}
