use x86_64::structures::paging::{PageSize, PhysFrame};

trait ToFrameNumeric<S: PageSize> {
    fn to_frame_numeric(&self) -> usize;
}

impl<S: PageSize> ToFrameNumeric<S> for PhysFrame<S> {
    //Phys 
    fn to_frame_numeric(&self) -> usize {
        self.start_address().as_u64() as usize
    }
}