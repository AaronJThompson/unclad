use core::marker::PhantomData;

use x86_64::{structures::paging::{page::AddressNotAligned, PageSize, PhysFrame}, PhysAddr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct FrameNumeric<S: PageSize> {
    pub num: usize,

    _marker: PhantomData<S>,
}

impl<S: PageSize> FrameNumeric<S> {
    pub const fn from_num(n: usize) -> Self {
        FrameNumeric {
            num: n,
            _marker: PhantomData,
        }
    }
}

impl<S: PageSize> TryFrom<usize> for FrameNumeric<S> {
    type Error = AddressNotAligned;
    
    fn try_from(value: usize) -> Result<Self, Self::Error> {
        if value % (S::SIZE as usize) != 0 {
            return Err(AddressNotAligned);
        }
        Ok(FrameNumeric {
            num: value / (S::SIZE as usize),
            _marker: PhantomData,
        })
    }
}

impl<S: PageSize> TryFrom<u64> for FrameNumeric<S> {
    type Error = AddressNotAligned;
    
    fn try_from(value: u64) -> Result<Self, Self::Error> {
        if value % S::SIZE != 0 {
            return Err(AddressNotAligned);
        }
        Ok(FrameNumeric {
            num: value as usize / (S::SIZE as usize),
            _marker: PhantomData,
        })
    }
}

impl<S: PageSize> From<PhysFrame<S>> for FrameNumeric<S> {
    fn from(frame: PhysFrame<S>) -> Self {
        FrameNumeric {
            num: frame.start_address().as_u64() as usize / (S::SIZE as usize),
            _marker: PhantomData,
        }
    }
}

impl<S: PageSize> From<FrameNumeric<S>> for PhysFrame<S> {
    fn from(frame: FrameNumeric<S>) -> Self {
        PhysFrame::from_start_address(frame.into()).unwrap()
    }
}

impl<S: PageSize> From<FrameNumeric<S>> for PhysAddr {
    #[inline]
    fn from(frame: FrameNumeric<S>) -> Self {
        PhysAddr::new(frame.num as u64 * S::SIZE)
    }
}

impl<S: PageSize> From<FrameNumeric<S>> for usize {
    fn from(frame: FrameNumeric<S>) -> Self {
        frame.num
    }
}

pub trait ToFrameNumeric<S: PageSize> {
    fn to_frame_numeric(&self) -> usize;
}

impl<S: PageSize> ToFrameNumeric<S> for PhysFrame<S> {

    #[inline]
    fn to_frame_numeric(&self) -> usize {
        self.start_address().as_u64() as usize / (S::SIZE as usize)
    }
}

impl<S: PageSize> ToFrameNumeric<S> for u64 {

    #[inline]
    fn to_frame_numeric(&self) -> usize {
        (*self as usize) / (S::SIZE as usize)
    }
}

macro_rules! assert_aligned {
    ($addr:expr, $size:expr) => {
        assert_eq!(x86_64::addr::align_down($addr, $size), $addr, "Address is not aligned to {}", $size);
    };
}

pub(crate) use assert_aligned;