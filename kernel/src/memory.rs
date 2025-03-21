use core::{error::Error, ops::Range};
use bootloader_api::info::{MemoryRegion, MemoryRegionKind, MemoryRegions};
use buddy_system_allocator::FrameAllocator;
use x86_64::{
    PhysAddr, VirtAddr,
    structures::paging::{
        Mapper, OffsetPageTable, Page, PageSize, PageTable, PageTableFlags,
        PhysFrame, Size4KiB, frame,
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
