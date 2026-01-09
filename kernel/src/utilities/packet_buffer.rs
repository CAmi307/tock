use crate::ErrorCode;
use core::any::Any;
use core::fmt::Debug;
use core::ops::{Range, RangeFrom};
use cortex_m_semihosting::hprintln;

/// Internal `PacketBufferDyn` trait, shared across various packet buffer
/// backends (such as [`PacketSlice`]).
///
/// This is a safe interface, but should not be used directly. Instead,
/// manipulate `PacketBufferDyn`s using the [`PacketBufferMut`] container.
pub unsafe trait PacketBufferDyn: Any + Debug {
    /// Length of the allocated data in this buffer (excluding head- and
    /// tailroom).
    fn len(&self) -> usize;

    /// Available headroom in the underlying buffer.
    fn headroom(&self) -> usize;

    /// Available tailroom in the underlying buffer.
    fn tailroom(&self) -> usize;

    /// Length of the writeable data in this buffer
    ///
    /// Equal to payload size + headroom available + tailroom available
    fn capacity(&self) -> usize;

    /// Force-reclaim a given amount of headroom in this buffer. This will
    /// ignore any current data stored in the buffer (but not immediately
    /// overwrite it). It will not move past the tailroom marker.
    ///
    /// This method returns a boolean indicating success. A `false` return value
    /// indicates that the `PacketBufferDyn` was not modified.
    fn reclaim_headroom(&mut self, new_headroom: usize) -> bool;

    fn reclaim_tailroom(&mut self, new_tailroom: usize) -> bool;

    /// Force-reset the payload to length `0`, and set a new headroom
    /// pointer. This will ensure that a subsequent prepend operation starts at
    /// this new headroom pointer.
    ///
    /// This method returns a boolean indicating success. It may fail if
    /// `new_headroom > self.headroom() + self.len() + self.tailroom()`. A
    /// `false` return value indicates that the `PacketBufferDyn` was not
    /// modified.
    fn reset(&mut self, new_headroom: usize) -> bool;

    fn copy_from_slice_or_err(&mut self, src: &[u8]) -> Result<(), ErrorCode>;

    fn append_from_slice_max(&mut self, src: &[u8]) -> usize;

    // has to be guaranteed to fit !!!!!
    unsafe fn prepand_unchecked(&mut self, header: &[u8]);

    fn payload(&self) -> &[u8];

    fn payload_mut(&mut self) -> &mut [u8];

    // fn iter_mut<'a>(&'a mut self) -> impl Iterator<Item = &mut u8> + 'a;
}

#[derive(Debug)]
pub struct PacketBufferMut {
    pub inner: &'static mut dyn PacketBufferDyn,
    pub current_constraints: (usize, usize),
    pub constraints: &'static [(usize, usize)],
    pub constraint_index: usize,
    pub component: &'static str,
}

impl PacketBufferMut {
    #[inline(always)]
    pub fn new(
        component: &'static str,
        inner: &'static mut dyn PacketBufferDyn,
        constraints: &'static [(usize, usize)],
    ) -> Option<Self> {
        let first_constraints = constraints[0];

        if inner.headroom() >= first_constraints.0 && inner.tailroom() >= first_constraints.1 {
            Some(PacketBufferMut {
                component,
                inner,
                constraints,
                current_constraints: first_constraints,
                constraint_index: 0,
            })
        } else {
            return None;
        }
    }

    /// Length of the allocated space for the structure
    ///
    ///
    /// Length of the allocated data in this buffer (excluding head- and
    /// tailroom).
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Actual available headroom in the underlying buffer. Must be greater or
    /// equal to the `HEAD` parameter.
    #[inline(always)]
    pub fn headroom(&self) -> usize {
        self.inner.headroom()
    }

    /// Actual available tailroom in the underlying buffer. Must be greater or
    /// equal to the `TAIL` parameter.
    #[inline(always)]
    pub fn tailroom(&self) -> usize {
        self.inner.tailroom()
    }

    pub fn capacity(&self) -> usize {
        self.inner.capacity()
    }

    #[inline(always)]
    pub fn downcast<T: PacketBufferDyn>(self) -> Option<&'static mut T> {
        let any_buffer: &'static mut dyn Any = self.inner as _;
        any_buffer.downcast_mut::<T>()
    }

    #[inline(never)]
    pub fn prepend(self, header: &[u8]) -> PacketBufferMut {
        let next = self.constraints[self.constraint_index + 1];
        assert!(
            next.0 == self.current_constraints.0 - header.len(),
            "Tried to prepend {} bytes. Current constraints are: head={}, tail={}. Next constraints are: head={}, tail={}",
            header.len(),
            self.current_constraints.0,
            self.current_constraints.1,
            next.0,
            next.1
        );

        unsafe {
            self.inner.prepand_unchecked(header);
        }

        // Re-build self with updated heads and tails
        let next_constraints_index = self.constraint_index + 1;
        Self {
            component: self.component,
            inner: self.inner,
            current_constraints: self.constraints[next_constraints_index],
            constraints: self.constraints,
            constraint_index: next_constraints_index,
        }
    }

    #[inline(never)]
    pub fn append(self, tail: &[u8]) -> PacketBufferMut {
        let next = self.constraints[self.constraint_index + 1];
        assert!(
            next.1 == self.current_constraints.1 - tail.len(),
            "Tried to append {} bytes. Current constraints are: head={}, tail={}. Next constraints are: head={}, tail={}",
            tail.len(),
            self.current_constraints.0,
            self.current_constraints.1,
            next.0,
            next.1
        );

        self.inner.append_from_slice_max(tail);

        // Re-build self with updated heads and tails
        let next_constraints_index = self.constraint_index + 1;
        Self {
            component: self.component,

            inner: self.inner,
            current_constraints: self.constraints[next_constraints_index],
            constraints: self.constraints,
            constraint_index: next_constraints_index,
        }
    }

    pub fn copy_from_slice_or_err(&mut self, src: &[u8]) -> Result<(), ErrorCode> {
        self.inner.copy_from_slice_or_err(src)
    }

    pub fn payload(&self) -> &[u8] {
        self.inner.payload()
    }

    pub fn payload_mut(&mut self) -> &mut [u8] {
        self.inner.payload_mut()
    }

    pub fn reclaim_previous_constraints(self) -> Result<PacketBufferMut, Self> {
        if self.constraint_index == 0 {
            return Err(self);
        }

        let previous_constraints_index = self.constraint_index - 1;
        let previous_constraints = self.constraints[previous_constraints_index];

        if self.inner.reclaim_headroom(previous_constraints.0)
            && self.inner.reclaim_tailroom(previous_constraints.1)
        {
            Ok(PacketBufferMut {
                component: self.component,
                inner: self.inner,
                current_constraints: previous_constraints,
                constraints: self.constraints,
                constraint_index: previous_constraints_index,
            })
        } else {
            Err(self)
        }
    }

    pub fn restore_previous_constraints(self) -> Result<PacketBufferMut, Self> {
        if self.constraint_index == 0 {
            return Err(self);
        }

        let previous_constraints_index = self.constraint_index - 1;
        let previous_constraints = self.constraints[previous_constraints_index];

        if self.inner.headroom() >= previous_constraints.0
            && self.inner.tailroom() >= previous_constraints.1
        {
            Ok(PacketBufferMut {
                component: self.component,
                inner: self.inner,
                current_constraints: previous_constraints,
                constraints: self.constraints,
                constraint_index: previous_constraints_index,
            })
        } else {
            Err(self)
        }
    }
}

// PacketSliceMut is a transparent wrapper around a byte, such that we
// can take a dyn reference to it (must be Sized). We create it from a
// slice by storing the slice's length in the first usize words and
// never modifying that.
#[repr(transparent)]
// TODO: should fix the debug trait
#[derive(Debug)]
pub struct PacketSliceMut {
    // Use the first `core::mem::size_of<usize>()` bytes as the
    // original slice length, second word as headroom, and the third
    // word as tailroom.
    _inner: u8,
}

impl PacketSliceMut {
    const SLICE_LENGTH_BYTES: Range<usize> =
        (0 * core::mem::size_of::<usize>())..(1 * core::mem::size_of::<usize>());
    const HEADROOM_BYTES: Range<usize> =
        (1 * core::mem::size_of::<usize>())..(2 * core::mem::size_of::<usize>());
    const TAILROOM_BYTES: Range<usize> =
        (2 * core::mem::size_of::<usize>())..(3 * core::mem::size_of::<usize>());
    const DATA_SLICE: RangeFrom<usize> = (3 * core::mem::size_of::<usize>())..;

    // TODO: horribly unsafe, check and document safety!
    pub fn new<'a>(
        slice: &'a mut [u8],
        headroom: usize,
    ) -> Result<&'a mut PacketSliceMut, &'a mut [u8]> {
        if slice.len() < Self::DATA_SLICE.start {
            Err(slice)
        } else {
            // Write the slice's length into its first word:
            let length = slice.len();
            slice[Self::SLICE_LENGTH_BYTES].copy_from_slice(&usize::to_ne_bytes(length));

            // Start with zero headroom, and full tailroom (simulating an empty slice)
            slice[Self::HEADROOM_BYTES].copy_from_slice(&usize::to_ne_bytes(headroom));
            slice[Self::TAILROOM_BYTES].copy_from_slice(&usize::to_ne_bytes(
                length - Self::DATA_SLICE.start - headroom,
            ));

            // Discard the slice, storing only a reference to its first
            // byte. The safety of this infrastructure depends on us having
            // written the correct length to the first word
            // (`SLICE_LENGTH_BYTES`), and us _never_ overwriting that word.
            Ok(
                unsafe {
                    core::mem::transmute::<&'a mut u8, &'a mut PacketSliceMut>(&mut slice[0])
                },
            )
        }
    }

    pub fn into_inner(&'static mut self) -> &'static mut [u8] {
        let length = self.get_inner_slice_length();
        unsafe { core::slice::from_raw_parts_mut(self as *mut _ as *mut u8, length) }
    }

    fn get_inner_slice_length(&self) -> usize {
        // We use this function for restoring the inner slice, and as such we
        // must avoid using those methods here. We're only interested in the
        // first word, and the slice is guaranteed to be of sufficient length
        // for that:
        let _: () = assert!(Self::SLICE_LENGTH_BYTES.start == 0);
        let length_slice = unsafe {
            core::slice::from_raw_parts(self as *const _ as *const u8, Self::SLICE_LENGTH_BYTES.end)
        };

        // The `length_slice` is guaranteed to have the correct length (one
        // usize word), so this panic should be elided:
        usize::from_ne_bytes(length_slice.try_into().unwrap())
    }

    fn restore_inner_slice<'a>(&'a self) -> &'a [u8] {
        let length = self.get_inner_slice_length();

        // `get_inner_slice_length` does not keep a reference to the underlying
        // memory in scope, so now construct the final slice with the correct
        // length:
        unsafe { core::slice::from_raw_parts(self as *const _ as *const u8, length) }
    }

    // TODO: document safety. Unsafe because the slice can be used to change the
    // `SLICE_LENGTH_BYTES` attribute.
    unsafe fn restore_inner_slice_mut<'a>(&'a mut self) -> &'a mut [u8] {
        let length = self.get_inner_slice_length();

        // `get_inner_slice_length` does not keep a reference to the underlying
        // memory in scope, so now construct the final slice with the correct
        // length:
        core::slice::from_raw_parts_mut(self as *mut _ as *mut u8, length)
    }

    pub fn get_headroom(&self) -> usize {
        usize::from_ne_bytes(
            self.restore_inner_slice()[Self::HEADROOM_BYTES]
                .try_into()
                .unwrap(),
        )
    }

    fn set_headroom(&mut self, headroom: usize) {
        unsafe {
            self.restore_inner_slice_mut()[Self::HEADROOM_BYTES]
                .copy_from_slice(&usize::to_ne_bytes(headroom));
        }
    }

    fn get_tailroom(&self) -> usize {
        usize::from_ne_bytes(
            self.restore_inner_slice()[Self::TAILROOM_BYTES]
                .try_into()
                .unwrap(),
        )
    }

    fn set_tailroom(&mut self, tailroom: usize) {
        unsafe {
            self.restore_inner_slice_mut()[Self::TAILROOM_BYTES]
                .copy_from_slice(&usize::to_ne_bytes(tailroom));
        }
    }

    pub fn data_slice<'a>(&'a self) -> &'a [u8] {
        let slice = self.restore_inner_slice();

        &slice[Self::DATA_SLICE]
    }

    pub fn data_slice_mut<'a>(&'a mut self) -> &'a mut [u8] {
        unsafe { &mut self.restore_inner_slice_mut()[Self::DATA_SLICE] }
    }
}

//     fn headroom_mut<'a>(&'a mut self) -> &'a mut usize {
// 	let slice = unsafe { core::mem::transmute::<&'a mut Self, &'a mut [u8]>(self) };
// 	unsafe { core::mem::transmute::<&'a mut u8, &'a mut usize>(
// 	    &mut slice[0 * core::mem::size_of::<usize>()]
// 	) }
//     }

//     fn tailroom_mut<'a>(&'a mut self) -> &'a mut usize {
// 	let slice = unsafe { core::mem::transmute::<&'a mut Self, &'a mut [u8]>(self) };
// 	unsafe { core::mem::transmute::<&'a mut u8, &'a mut usize>(
// 	    &mut slice[1 * core::mem::size_of::<usize>()]
// 	) }
//     }

//     fn data_mut<'a>(&'a mut self) -> &'a mut [u8] {
// 	let slice = unsafe { core::mem::transmute::<&'a mut Self, &'a mut [u8]>(self) };
// 	&mut slice[2 * core::mem::size_of::<usize>()..]
//     }

//     fn headroom_mut<'a>(&'a mut self) -> &'a mut usize {
// 	let slice = unsafe { core::mem::transmute::<&'a mut Self, &'a mut [u8]>(self) };
// 	unsafe { core::mem::transmute::<&'a mut u8, &'a mut usize>(
// 	    &mut slice[0 * core::mem::size_of::<usize>()]
// 	) }
//     }

//     fn tailroom_mut<'a>(&'a mut self) -> &'a mut usize {
// 	let slice = unsafe { core::mem::transmute::<&'a mut Self, &'a mut [u8]>(self) };
// 	unsafe { core::mem::transmute::<&'a mut u8, &'a mut usize>(
// 	    &mut slice[1 * core::mem::size_of::<usize>()]
// 	) }
//     }

//     fn data_mut<'a>(&'a mut self) -> &'a mut [u8] {
// 	let slice = unsafe { core::mem::transmute::<&'a mut Self, &'a mut [u8]>(self) };
// 	&mut slice[2 * core::mem::size_of::<usize>()..]
//     }
// }

unsafe impl PacketBufferDyn for PacketSliceMut {
    fn len(&self) -> usize {
        // self.data_slice().len() - self.headroom() - self.tailroom()
        self.get_inner_slice_length()
    }

    fn headroom(&self) -> usize {
        self.get_headroom()
    }

    fn tailroom(&self) -> usize {
        self.get_tailroom()
    }

    fn capacity(&self) -> usize {
        self.data_slice().len()
    }

    fn reclaim_headroom(&mut self, new_headroom: usize) -> bool {
        if new_headroom <= self.data_slice().len() - self.tailroom() {
            self.set_headroom(new_headroom);
            true
        } else {
            false
        }
    }

    fn reclaim_tailroom(&mut self, new_tailroom: usize) -> bool {
        if new_tailroom <= self.data_slice().len() - self.headroom() {
            self.set_tailroom(new_tailroom);
            true
        } else {
            false
        }
    }

    fn reset(&mut self, new_headroom: usize) -> bool {
        if new_headroom > self.data_slice().len() {
            false
        } else {
            self.set_headroom(new_headroom);
            self.set_tailroom(self.data_slice().len() - new_headroom);
            true
        }
    }

    fn copy_from_slice_or_err(&mut self, src: &[u8]) -> Result<(), ErrorCode> {
        let headroom: usize = self.get_headroom();
        let available: &mut [u8] = &mut self.data_slice_mut()[headroom..];
        if available.len() < src.len() {
            Err(ErrorCode::SIZE)
        } else {
            available
                .iter_mut()
                .zip(src.iter())
                .for_each(|(dst, src)| *dst = *src);
            self.set_tailroom(self.data_slice().len() - self.get_headroom() - src.len());
            Ok(())
        }
    }

    fn append_from_slice_max(&mut self, src: &[u8]) -> usize {
        let slice_length = self.data_slice().len();
        let tailroom = self.get_tailroom();
        let offset = slice_length - tailroom;
        let count = core::cmp::min(tailroom, src.len());

        self.data_slice_mut()[offset..(offset + count)]
            .iter_mut()
            .zip(src[..count].iter())
            .for_each(|(dst, src)| *dst = *src);

        self.set_tailroom(tailroom - count);
        count
    }

    unsafe fn prepand_unchecked(&mut self, header: &[u8]) {
        self.set_headroom(self.get_headroom().saturating_sub(header.len()));
        let headroom = self.get_headroom();
        self.data_slice_mut()[headroom..headroom + header.len()].copy_from_slice(header);
    }

    fn payload(&self) -> &[u8] {
        let headroom = self.headroom();
        let tailroom = self.tailroom();
        let capacity = self.capacity();

        //     capacity,
        //     headroom,
        //     tailroom
        // );

        &self.data_slice()[headroom..(capacity - tailroom)]
    }

    fn payload_mut(&mut self) -> &mut [u8] {
        let headroom = self.headroom();
        let tailroom = self.tailroom();
        let capacity = self.capacity();

        &mut self.data_slice_mut()[headroom..(capacity - tailroom)]
    }
    // TODO same for tail

    // fn iter_mut<'a>(&'a mut self) -> impl core::slice::IterMut<Item = &mut u8> + 'a {
    // 	let headroom = self.get_headroom();
    // 	let tailroom = self.get_tailroom();
    // 	let slice = self.data_slice_mut();
    // 	let length = slice.len();
    // 	slice[headroom..(length - tailroom)].iter_mut()
    // }
}
