pub struct VtDecoder;

impl VtDecoder {
    pub fn new() -> Option<Self> {
        None
    }

    pub fn take_latest(&self) -> Option<(u32, u32, Vec<u8>)> {
        None
    }

    pub fn feed(&mut self, _au: &[u8]) -> bool {
        false
    }
}
