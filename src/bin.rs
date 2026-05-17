use std::ops::Deref;

use uom::si::information::byte;

use crate::information::Information;

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
    pub(crate) page_size: Information,
}

impl MemoryBin {
    pub(crate) fn num_pages(&self) -> u64 {
        (self.end_address - self.start_address) / self.page_size.get::<byte>()
    }
}

impl Deref for Bin {
    type Target = [MemoryBin];

    fn deref(&self) -> &Self::Target {
        self.bins.deref()
    }
}
