use crate::profile::TargetProfile;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbiConvention;

impl AbiConvention {
    pub fn for_profile(_profile: &TargetProfile) -> Self {
        AbiConvention
    }

    pub fn cells_needed(&self, profile: &TargetProfile, bit_width: u32) -> u32 {
        profile.cells_for_type(bit_width)
    }

    pub fn argument_cell_sequence(&self, profile: &TargetProfile, param_bit_widths: &[u32]) -> u32 {
        param_bit_widths
            .iter()
            .map(|&w| self.cells_needed(profile, w))
            .sum()
    }
}
