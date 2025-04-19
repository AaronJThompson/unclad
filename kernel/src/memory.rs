use core::{error::Error, ops::Range};
use bootloader_api::info::{MemoryRegion, MemoryRegionKind, MemoryRegions};
use buddy_system_allocator::FrameAllocator;
use x86_64::{
    PhysAddr, VirtAddr,
    structures::paging::{
        Mapper, OffsetPageTable, Page, PageSize, PageTable, PageTableFlags,
        PhysFrame, Size4KiB, frame, FrameAllocator as FrameAllocatorTrait,
    },
};

use crate::{ALLOC_ORDER, HEAP, PHYS_OFFSET};

pub unsafe fn active_level_4_table(physical_memory_offset: VirtAddr) -> &'static mut PageTable {
    use x86_64::registers::control::Cr3;

    let (level_4_table_frame, _) = Cr3::read();

    let phys = level_4_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    unsafe { &mut *page_table_ptr }
}

pub unsafe fn get_active_opt(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    let l4_table = active_level_4_table(physical_memory_offset);
    OffsetPageTable::new(l4_table, physical_memory_offset)
}

//TODO: Make const at somepoint once we can statically know offset
pub fn phys_to_virt(addr: PhysAddr) -> VirtAddr {
    let offset = unsafe{ *PHYS_OFFSET.get_unchecked() };
    let virt_addr = addr.as_u64() + offset as u64;
    VirtAddr::new(virt_addr)
}

pub struct FrameAllocatorWrapper<'a>(pub(crate) &'a mut FrameAllocator<ALLOC_ORDER>);

unsafe impl<S: PageSize> FrameAllocatorTrait<S> for FrameAllocatorWrapper<'_> {
    fn allocate_frame(&mut self) -> Option<PhysFrame<S>> {
        let frame_num = self.0.alloc(1).unwrap();
        Some(PhysFrame::from_start_address(PhysAddr::new(frame_num as u64 * S::SIZE)).unwrap())
    }
}

pub fn allocate_heap<S: PageSize, const O: usize>(
    frame_allocator: &mut FrameAllocator<O>,
) {
    const HEAP_SIZE: u64 = 1024 * 1024; // 1Mib
    log::debug!("Allocating frames for heap");
    let start_frame = frame_allocator.alloc((HEAP_SIZE / S::SIZE) as usize).unwrap();
    log::debug!("Heap start frame: {}", start_frame);
    let start_addr =  start_frame * S::SIZE as usize;
    let mut heap_lock = HEAP.lock();
    log::debug!("Initializing heap");
    unsafe { heap_lock.add_to_heap(PHYS_OFFSET.get().unwrap() + start_addr,PHYS_OFFSET.get().unwrap() + start_addr + HEAP_SIZE as usize)} ;
}

pub fn assign_frames<S: PageSize, const O: usize>(mr: &MemoryRegions, frame_allocator: &mut FrameAllocator<O>) {
    let regions = mr.iter();
    let usable_regions = regions.filter(|r| r.kind == MemoryRegionKind::Usable);
    // map each region to its address range
    let mut usable_frame_ranges = usable_regions.map(|r| {
        PhysFrame::<S>::range_inclusive(
            PhysFrame::containing_address(PhysAddr::new(r.start)),
            PhysFrame::containing_address(PhysAddr::new(r.end)),
        )
    });
    log::info!("Assigning frames");
    let mut heap_allocated = false;
    for frame_range in usable_frame_ranges.filter(|r| r.start.start_address().as_u64() != 0x8000) {
        let mut frame_start = frame_range.start.start_address().as_u64() / S::SIZE;
        let frame_end = frame_range.end.start_address().as_u64() / S::SIZE;
        if !heap_allocated {
                let start_addr = frame_start * S::SIZE;
                let mut guard = HEAP.lock();
                unsafe { guard.init(PHYS_OFFSET.get().unwrap() + start_addr as usize, 4 * S::SIZE as usize)};
                drop(guard);
                log::debug!("Heap pre-allocated with 1 frame");
                heap_allocated = true;
                frame_start += 1;
        }
        if frame_start >= frame_end {
            continue;
        }
        frame_allocator.add_frame(frame_start as usize, frame_end as usize);
    }
}


// At most 10 bits can be set in the flags field, so we must guarantee this
#[derive(Debug, Clone, Copy)]
pub struct StackRef(u16);
impl StackRef {
    const MAX_BITS: u16 = 10;
    pub const fn new(n: u16) -> Option<Self> {
        if (n >> StackRef::MAX_BITS) != 0 {
            return None;
        }
        Some(StackRef(n))
    }

    pub const fn as_u16(&self) -> u16 {
        self.0
    }
}

pub struct TooLargeError;

impl TryFrom<u16> for StackRef {
    type Error = TooLargeError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match StackRef::new(value) {
            Some(v) => Ok(v),
            None => Err(TooLargeError)
        }
    }
}

#[const_trait]
pub(crate) trait UncladCustomPageFlags {
    #[must_use]
    fn mark_as_stack(&self) -> Self;
    fn is_stack(&self) -> bool;
    #[must_use]
    fn mark_as_guard(&self) -> Self;
    fn is_guard(&self) -> bool;
    #[must_use]
    fn assign_stack_ref(&self, stack_ref: StackRef) -> Self;
}

impl const UncladCustomPageFlags for PageTableFlags {
    fn mark_as_stack(&self) -> Self {
        Self::union(*self, PageTableFlags::BIT_9)
    }

    fn is_stack(&self) -> bool {
        self.contains(PageTableFlags::BIT_9)
    }

    fn mark_as_guard(&self) -> Self{
        Self::union(*self, PageTableFlags::BIT_10)
    }

    fn is_guard(&self) -> bool {
        self.contains(PageTableFlags::BIT_10)
    }

    fn assign_stack_ref(&self, stack_ref: StackRef) -> Self {
        //SAFTEY: We know that the value is less than 10 bits
        let val = (stack_ref.as_u16() as u64) << 52;
        Self::union(*self,PageTableFlags::from_bits(val).unwrap())
    }
}
