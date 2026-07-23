use crate::profile::TargetProfile;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryLayout {
    pub global_base: u32,
    pub heap_base: u32,
    pub stack_base: u32,
    pub memory_size: u32,
}

impl MemoryLayout {
    pub fn for_profile(_profile: &TargetProfile, num_global_bytes: u32, memory_size: u32) -> Self {
        let global_base = 1u32;
        let heap_base = global_base + num_global_bytes;
        MemoryLayout {
            global_base,
            heap_base,
            stack_base: memory_size,
            memory_size,
        }
    }

    pub fn max_addressable(&self, profile: &TargetProfile) -> u64 {
        profile.max_addressable_bytes()
    }
}
