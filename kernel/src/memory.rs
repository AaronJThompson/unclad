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

use crate::{HEAP, PHYS_OFFSET};

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

// /// A FrameAllocator that returns usable frames from the bootloader's memory map.
// pub struct RegionalFrameAllocator<S: PageSize> {
//     memory_map: &'static MemoryRegions,
//     next: usize,
//     _marker: core::marker::PhantomData<S>,
// }

// impl<S: PageSize> RegionalFrameAllocator<S> {
//     /// Returns an iterator over the usable memory ranges from the memory map.
//     fn usable_ranges(&self) -> impl Iterator<Item = Range<u64>> {
//         // get usable regions from memory map
//         let regions = self.memory_map.iter();
//         let usable_regions = regions.filter(|r| r.kind == MemoryRegionKind::Usable);
//         // map each region to its address range
//         usable_regions.map(|r| r.start..r.end)
//     }

//     pub fn new(mr: &'static MemoryRegions) -> Self {
//         RegionalFrameAllocator {
//             memory_map: mr,
//             next: 0,
//             _marker: core::marker::PhantomData,
//         }
//     }
// }

// unsafe impl<S: PageSize> FrameAllocator<S> for RegionalFrameAllocator<S> {
//     fn allocate_frame(&mut self) -> Option<PhysFrame<S>> {
//         let frame = self.usable_frames().nth(self.next);
//         self.next += 1;
//         frame
//     }
// }

pub struct FrameAllocatorWrapper<'a>(pub(crate) &'a mut FrameAllocator<32>);

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
    for frame_range in usable_frame_ranges {
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
    pub fn new(n: u16) -> Option<Self> {
        if (n & 0xF << StackRef::MAX_BITS) != 0 {
            return None;
        }
        Some(StackRef(n))
    }
    pub fn get(&self) -> u16 {
        self.0
    }
    pub fn set(&mut self, val: u16) {
        self.0 = val;
    }
}

impl Into<u16> for StackRef {
    fn into(self) -> u16 {
        self.0
    }
}

struct TooLargeError;

impl TryFrom<u16> for StackRef {
    type Error = TooLargeError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        if (value & 0xF << StackRef::MAX_BITS) != 0 {
            Err(TooLargeError)
        } else {
            Ok(StackRef(value))
        }
    }
}

pub(crate) trait UncladCustomPageFlags {
    fn mark_as_stack(&mut self) -> &mut Self;
    fn is_stack(&self) -> bool;
    fn mark_as_guard(&mut self) -> &mut Self;
    fn is_guard(&self) -> bool;
    fn assign_stack_ref(&mut self, stack_ref: StackRef) -> &mut Self;
}

impl UncladCustomPageFlags for PageTableFlags {
    fn mark_as_stack(&mut self) -> &mut Self {
        self.set(PageTableFlags::BIT_9, true);
        self
    }

    fn is_stack(&self) -> bool {
        self.contains(PageTableFlags::BIT_9)
    }

    fn mark_as_guard(&mut self) -> &mut Self{
        self.set(PageTableFlags::BIT_10, true);
        self
    }

    fn is_guard(&self) -> bool {
        self.contains(PageTableFlags::BIT_10)
    }

    fn assign_stack_ref(&mut self, stack_ref: StackRef) -> &mut Self {
        //SAFTEY: We know that the value is less than 10 bits
        let val = (stack_ref.get() as u64) << 52;
        self.set(PageTableFlags::from_bits(val).unwrap(), true);
        self
    }
}
