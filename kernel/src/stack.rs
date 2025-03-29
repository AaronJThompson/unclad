use core::ops::Index;

use x86_64::{
    PhysAddr, VirtAddr, align_up,
    structures::paging::{
        Mapper, Page, PageSize, PageTableFlags, PhysFrame, mapper::MapToError,
        page::AddressNotAligned,
    },
};

use crate::{
    FRAME_ALLOC, MAX_PROC_COUNT,
    memory::{FrameAllocatorWrapper, StackRef, UncladCustomPageFlags},
    x86_ext::{FrameNumeric, ToFrameNumeric, assert_aligned},
};

pub(crate) static mut STACK_REFS: [Stack; MAX_PROC_COUNT] = [Stack::empty(); MAX_PROC_COUNT];

const STACK_PAGE_FLAGS: PageTableFlags =
    PageTableFlags::union(PageTableFlags::WRITABLE, PageTableFlags::PRESENT).mark_as_stack();
const STACK_GUARD_FLAGS: PageTableFlags = PageTableFlags::empty().mark_as_stack().mark_as_guard();

const fn invalid_physframe<S: PageSize>() -> PhysFrame<S> {
    // SAFETY: This is intentionally invalid
    unsafe { PhysFrame::from_start_address_unchecked(PhysAddr::new(0)) }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Stack {
    pub(crate) stack_ref: StackRef,
    pub(crate) stack_base: VirtAddr,
    pub(crate) max_stack_size: usize,
}

impl Stack {
    pub const fn empty() -> Self {
        Self {
            stack_ref: StackRef::new(0).unwrap(),
            stack_base: VirtAddr::zero(),
            max_stack_size: 0,
        }
    }
}

impl Index<StackRef> for [Stack] {
    type Output = Stack;
    fn index(&self, idx: StackRef) -> &Self::Output {
        &self[idx.as_u16() as usize]
    }
}

pub enum StackAllocError<S: PageSize> {
    OutOfFrames,
    UnableToMap(MapToError<S>),
    AddressNotAligned,
}

impl<S: PageSize> From<MapToError<S>> for StackAllocError<S> {
    fn from(err: MapToError<S>) -> Self {
        StackAllocError::UnableToMap(err)
    }
}

impl<S: PageSize> From<AddressNotAligned> for StackAllocError<S> {
    fn from(_: AddressNotAligned) -> Self {
        StackAllocError::AddressNotAligned
    }
}


pub fn alloc_stack_with_guard<M: Mapper<S>, S: PageSize>(
    initial_size: u64,
    mut mapper: M,
    addr: VirtAddr,
    stack_ref: StackRef,
) -> Result<PhysFrame<S>, StackAllocError<S>> {
    assert_aligned!(initial_size as u64, S::SIZE);

    let mut frame_alloc = unsafe { FRAME_ALLOC.get().unwrap().lock() };
    let initial_page_count: FrameNumeric<S> = initial_size.try_into()?;
    let first_frame_num = FrameNumeric::from_num(
        frame_alloc
            .alloc(initial_page_count.into())
            .ok_or(StackAllocError::OutOfFrames)?,
    );
    let first_frame = first_frame_num.into();
    let mut frame_alloc = FrameAllocatorWrapper(&mut *frame_alloc);
    for i in 0..initial_page_count.into() {
        let page = Page::containing_address(addr + (i as u64 * S::SIZE));
        let frame_num = FrameNumeric::from_num(first_frame_num.num + i);
        let frame = frame_num.into();
        unsafe {
            mapper.map_to(
                page,
                frame,
                STACK_PAGE_FLAGS.assign_stack_ref(stack_ref),
                &mut frame_alloc,
            )?;
        }
    }
    let addr_offset: PhysAddr = initial_page_count.into();
    let guard_page = Page::containing_address(addr + addr_offset.as_u64());
    unsafe {
        //CHECK: Will this even page fault?
        mapper.map_to(
            guard_page,
            invalid_physframe(),
            STACK_GUARD_FLAGS.assign_stack_ref(stack_ref),
            &mut frame_alloc,
        )?;
    }

    Ok(first_frame)
}
