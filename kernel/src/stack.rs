use x86_64::{align_up, structures::paging::{Mapper, Page, PageSize, PageTableFlags, PhysFrame}, PhysAddr, VirtAddr};

use crate::{memory::{FrameAllocatorWrapper, StackRef, UncladCustomPageFlags}, FRAME_ALLOC, MAX_PROC_COUNT};

pub(crate) static mut STACK_REFS: [VirtAddr; MAX_PROC_COUNT] = [VirtAddr::zero(); MAX_PROC_COUNT];

pub fn allocate_stacks() {

}

pub fn alloc_stack_with_guard<M: Mapper<S>, S: PageSize>(initial_size: usize, mut mapper: M, addr: VirtAddr, stack_ref: StackRef) -> PhysAddr {
    let aligned_stack_size = align_up(initial_size as _, S::SIZE);
    let mut frame_alloc = unsafe { FRAME_ALLOC.get().unwrap().lock() };
    let initial_page_count = aligned_stack_size / S::SIZE;
    let first_frame = frame_alloc.alloc(initial_page_count as _).unwrap();
    let first_frame_addr = PhysAddr::new(first_frame as u64 * S::SIZE);
    let mut frame_alloc = FrameAllocatorWrapper(&mut *frame_alloc);
    for i in 0..initial_page_count {
        let page = Page::containing_address(addr + (i * S::SIZE as u64));
        let frame_addr = PhysAddr::new((first_frame as u64 + i as u64) * S::SIZE);
        let frame = PhysFrame::from_start_address(frame_addr).unwrap();
        unsafe {
            //TODO: Handle error
            mapper.map_to(page, frame,*(PageTableFlags::WRITABLE | PageTableFlags::PRESENT).mark_as_stack().assign_stack_ref(stack_ref), &mut frame_alloc);
        }
    }
    let guard_page = Page::containing_address(addr + (initial_page_count * S::SIZE as u64));
    let frame_addr = PhysAddr::new((first_frame as u64 + initial_page_count as u64) * S::SIZE);
    let frame = PhysFrame::from_start_address(frame_addr).unwrap();
    unsafe {
        //TODO: Handle error
        mapper.map_to(guard_page, frame,*PageTableFlags::empty().mark_as_stack().mark_as_guard().assign_stack_ref(stack_ref), &mut frame_alloc);
    }
    
    first_frame_addr
}