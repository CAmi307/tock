use core::any::Any;

/// Base PacketBuffer trait. Implemented by all elements of a PacketBuffer chain
/// (linked list). Generic over whether it is `CONTIGUOUS`, its guaranteed
/// headroom reservation `HDR_RSV`, and the list's tail type.
// Indicating the Any supertrait is important, to be able to use Any's
// downcast_ref method to convert a trait object of this type back
// into its original type.
pub trait PacketBuffer: Any {
    fn len(&self) -> usize;
}

impl<T: PacketBuffer + Any + ?Sized> PacketBuffer for &'static T {
    fn len(&self) -> usize {
        (**self).len()
    }
}

impl<T: PacketBuffer + Any + ?Sized> PacketBuffer for &'static mut T {
    fn len(&self) -> usize {
        (**self).len()
    }
}

#[repr(transparent)]
pub struct PacketBufferMut<const CONTIGUOUS: bool, const HDR_RSV: usize> {
    inner: &'static mut dyn PacketBuffer,
}

impl<const CONTIGUOUS: bool, const HDR_RSV: usize> PacketBufferMut<CONTIGUOUS, HDR_RSV> {
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    #[inline(always)]
    pub fn reduce_headroom<const NEW_HDR_RSV: usize>(
        self,
    ) -> PacketBufferMut<CONTIGUOUS, NEW_HDR_RSV> {
        let _: () = assert!(HDR_RSV >= NEW_HDR_RSV);
        PacketBufferMut { inner: self.inner }
    }

    // In future versions of Rust we should be able to eliminate the additional
    // NEW_HDR_RSV argument and calculate it automatically.
    #[inline(always)]
    pub fn prepend<const LEN: usize, const NEW_HDR_RSV: usize>(
        self,
        _data: &[u8; LEN],
    ) -> PacketBufferMut<CONTIGUOUS, NEW_HDR_RSV> {
        let _: () = assert!(HDR_RSV.checked_sub(LEN).unwrap() == NEW_HDR_RSV);
        unimplemented!()
    }

    #[inline(always)]
    pub fn restore_headroom<const NEW_HDR_RSV: usize>(
        self,
    ) -> Option<PacketBufferMut<CONTIGUOUS, NEW_HDR_RSV>> {
        if self.len() >= NEW_HDR_RSV {
            Some(PacketBufferMut { inner: self.inner })
        } else {
            None
        }
    }

    #[inline(always)]
    pub fn downcast<T: PacketBuffer>(self) -> Option<&'static mut T> {
        let any_buffer: &'static mut dyn Any = self.inner as _;
        any_buffer.downcast_mut::<T>()
    }

    #[inline(always)]
    pub fn into_dyn_mut(self) -> &'static mut dyn PacketBuffer {
        self.inner
    }
}

impl PacketBufferMut<true, 0> {
    #[inline(always)]
    pub fn from_mut_slice_contiguous(
        ps: &'static mut PacketSlice<MutablePacketSliceTy, PacketBufferEnd>,
    ) -> Self {
        PacketBufferMut { inner: ps }
    }
}

impl PacketBufferMut<false, 0> {
    #[inline(always)]
    pub fn from_mut_slice<N: PacketBuffer>(
        ps: &'static mut PacketSlice<MutablePacketSliceTy, N>,
    ) -> Self {
        PacketBufferMut { inner: ps }
    }
}

impl<const HDR_RSV: usize> PacketBufferMut<true, HDR_RSV> {
    #[inline(always)]
    pub fn from_mut_arr_contiguous<const LEN: usize>(
        ps: &'static mut PacketArr<LEN, PacketBufferEnd>,
    ) -> Self {
        let _: () = assert!(HDR_RSV <= LEN);
        PacketBufferMut { inner: ps }
    }
}

impl<const HDR_RSV: usize> PacketBufferMut<false, HDR_RSV> {
    #[inline(always)]
    pub fn from_mut_arr<const LEN: usize, N: PacketBuffer>(
        ps: &'static mut PacketArr<LEN, N>,
    ) -> Self {
        let _: () = assert!(HDR_RSV <= LEN);
        PacketBufferMut { inner: ps }
    }
}

// impl<
//         const CONTIGUOUS: bool,
//         const HDR_RSV: usize,
//         T: PacketBuffer<CONTIGUOUS, HDR_RSV> + ?Sized,
//     > PacketBuffer<CONTIGUOUS, HDR_RSV> for &T
// {
//     type ShrunkOut<const NEW_HDR_RSV: usize> = T::ShrunkOut<NEW_HDR_RSV>;

//     fn len(&self) -> usize {
// 	(*self).len()
//     }

//     fn shrink_headroom<
// 	's,
// 	const NEW_HDR_RSV: usize,
// 	>(
// 	s: &'s mut Self,
//     ) -> &'s mut Self::ShrunkOut<NEW_HDR_RSV> {
// 	T::shrink_headroom::<NEW_HDR_RSV>(s)
//     }
// }

/// Dummy type denoting the end of a `PacketBuffer` chain. Cannot hold any data,
/// always contiguous, zero headroom reservation.
pub struct PacketBufferEnd;
impl PacketBuffer for PacketBufferEnd {
    fn len(&self) -> usize {
        0
    }
}
impl PacketBufferEnd {
    pub const fn new() -> Self {
        PacketBufferEnd
    }
}

// For convenience, we provide a const instantiation of this type.
pub const PACKET_BUFFER_END: PacketBufferEnd = PacketBufferEnd::new();

/// Slice packet buffer trait. This allows us to implement only a single
/// `PacketSlice`, which is generic over a `PacketSliceTy` that is either
/// mutable or immutable.
// TODO: sealed trait?
pub trait PacketSliceTy {
    fn len(&self) -> usize;
}

/// Mutable `PacketSliceTy`.
pub struct MutablePacketSliceTy(&'static mut [u8]);
impl PacketSliceTy for MutablePacketSliceTy {
    fn len(&self) -> usize {
        self.0.len()
    }
}

/// Immutable `PacketSliceTy`.
struct ImmutablePacketSliceTy(&'static [u8]);
impl PacketSliceTy for ImmutablePacketSliceTy {
    fn len(&self) -> usize {
        self.0.len()
    }
}

/// `PacketBuffer` slice element. Must be generic over the `CONTIGUOUS` and
/// `HDR_RSV` const generic attributes of the _next_ type for Rust reasons.
pub struct PacketSlice<S: PacketSliceTy + 'static, N: PacketBuffer> {
    slice: S,
    next: N,
}

/// If the next element in the `PacketBuffer` chain is the end
/// element, mark this slice as contiguous.
impl<S: PacketSliceTy + 'static, N: PacketBuffer> PacketBuffer for PacketSlice<S, N> {
    fn len(&self) -> usize {
        self.slice.len() + self.next.len()
    }
}

impl PacketSlice<ImmutablePacketSliceTy, PacketBufferEnd> {
    pub fn from_slice_end(slice: &'static [u8]) -> Self {
        PacketSlice {
            slice: ImmutablePacketSliceTy(slice),
            next: PacketBufferEnd::new(),
        }
    }
}

impl PacketSlice<MutablePacketSliceTy, PacketBufferEnd> {
    pub fn from_slice_end(slice: &'static mut [u8]) -> Self {
        PacketSlice {
            slice: MutablePacketSliceTy(slice),
            next: PacketBufferEnd::new(),
        }
    }
}

// trait ResizePacketBufferHeadroom<
// 	const CONTIGUOUS: bool,
// 	const HDR_RSV: usize,
// 	PB: PacketBuffer<CONTIGUOUS, HDR_RSV> + ?Sized
// > {
//     /// Support shrinking the packet slice headroom.
//     fn shrink_packet_slice_headroom<
// 	    'a,
// 	'b,
// 	's,
// 	// const CONTIGUOUS: bool,
// 	const NEW_HDR_RSV: usize,
// 	// const HDR_RSV: usize,
// 	const NEXT_CONTIGUOUS: bool,
// 	const NEXT_HDR_RSV: usize,
// 	P: PacketSliceTy<'a>,
// 	N: PacketBuffer<NEXT_CONTIGUOUS, NEXT_HDR_RSV> + 'b,
// 	>(
// 	s: &'s mut PacketSlice<'a, 'b, CONTIGUOUS, HDR_RSV, NEXT_CONTIGUOUS, NEXT_HDR_RSV, P, N>,
//     ) -> &'s mut PacketSlice<'a, 'b, CONTIGUOUS, NEW_HDR_RSV, NEXT_CONTIGUOUS, NEXT_HDR_RSV, P, N> {
// 	let _: () = assert!(HDR_RSV >= NEW_HDR_RSV);
// 	unsafe { core::mem::transmute(s) }
//     }
// }

/// Support shrinking the packet slice headroom.
// fn shrink_packet_buffer_headroom<
//     's,
//     const CONTIGUOUS: bool,
//     const NEW_HDR_RSV: usize,
//     const HDR_RSV: usize,
// >(
//     s: &'s mut dyn PacketBuffer<CONTIGUOUS, HDR_RSV>,
// ) -> &'s mut dyn PacketBuffer<CONTIGUOUS, NEW_HDR_RSV> {
//     let _: () = assert!(HDR_RSV >= NEW_HDR_RSV);
//     unsafe { core::mem::transmute(s) }
// }

// /// Support shrinking the packet slice headroom.
// fn restore_packet_buffer_headroom<
//     's,
//     const CONTIGUOUS: bool,
//     const NEW_HDR_RSV: usize,
//     const HDR_RSV: usize,
// >(
//     s: &'s mut dyn PacketBuffer<CONTIGUOUS, HDR_RSV>,
// ) -> Option<&'s mut dyn PacketBuffer<CONTIGUOUS, NEW_HDR_RSV>> {
//     // TODO: check that we don't go beyond the length!
//     Some(unsafe { core::mem::transmute(s) })
// }

/// Support the headroom up to the entire packetbuffer's length.
// fn restore_packet_slice_headroom<
//     'a,
//     'b,
//     's,
//     const CONTIGUOUS: bool,
//     const NEW_HDR_RSV: usize,
//     const HDR_RSV: usize,
//     const NEXT_CONTIGUOUS: bool,
//     const NEXT_HDR_RSV: usize,
//     P: PacketSliceTy<'a>,
//     N: PacketBuffer<NEXT_CONTIGUOUS, NEXT_HDR_RSV> + 'b,
// >(
//     s: &'s mut PacketSlice<'a, 'b, CONTIGUOUS, HDR_RSV, NEXT_CONTIGUOUS, NEXT_HDR_RSV, P, N>,
// ) -> Option<&'s mut PacketSlice<'a, 'b, CONTIGUOUS, NEW_HDR_RSV, NEXT_CONTIGUOUS, NEXT_HDR_RSV, P, N>> {
//     if NEW_HDR_RSV <= s.len() {
// 	Some(unsafe { core::mem::transmute(s) })
//     } else {
// 	None
//     }
// }

impl<N: PacketBuffer> PacketSlice<ImmutablePacketSliceTy, N> {
    pub fn from_slice(slice: &'static [u8], next: N) -> Self {
        PacketSlice {
            slice: ImmutablePacketSliceTy(slice),
            next,
        }
    }
}

impl PacketSlice<MutablePacketSliceTy, PacketBufferEnd> {
    pub fn from_slice_mut_end(slice: &'static mut [u8]) -> Self {
        PacketSlice {
            slice: MutablePacketSliceTy(slice),
            next: PacketBufferEnd::new(),
        }
    }
}

impl<N: PacketBuffer> PacketSlice<MutablePacketSliceTy, N> {
    pub fn from_slice_mut(slice: &'static mut [u8], next: N) -> Self {
        PacketSlice {
            slice: MutablePacketSliceTy(slice),
            next,
        }
    }
}

pub struct PacketArr<const LEN: usize, N: PacketBuffer> {
    arr: [u8; LEN],
    next: N,
}

impl<const LEN: usize, N: PacketBuffer> PacketBuffer for PacketArr<LEN, N> {
    fn len(&self) -> usize {
        self.arr.len() + self.next.len()
    }
}

impl<const LEN: usize> PacketArr<LEN, PacketBufferEnd> {
    pub fn from_arr_end(arr: [u8; LEN]) -> Self {
        PacketArr {
            arr,
            next: PacketBufferEnd::new(),
        }
    }
}

impl<const LEN: usize, N: PacketBuffer> PacketArr<LEN, N> {
    pub fn from_arr(arr: [u8; LEN], next: N) -> Self {
        PacketArr { arr, next }
    }
}

impl<const LEN: usize, N: PacketBuffer> PacketArr<LEN, N> {
    pub fn into_inner(self) -> ([u8; LEN], N) {
        (self.arr, self.next)
    }

    pub fn inner(&self) -> &[u8; LEN] {
        &self.arr
    }

    pub fn inner_mut(&mut self) -> &[u8; LEN] {
        &mut self.arr
    }
}

// struct BufferTypeCapture<
//     const CONTIGUOUS: bool,
//     const HDR_RSV: usize,
//     T: PacketBuffer<CONTIGUOUS, HDR_RSV>,
// > {
//     state: core::cell::Cell<bool>,
//     _pd: T,
// }

// impl<const CONTIGUOUS: bool, const HDR_RSV: usize, T: PacketBuffer<CONTIGUOUS, HDR_RSV>>
//     BufferTypeCapture<CONTIGUOUS, HDR_RSV, T>
// {
//     pub fn capture(
//         &self,
//         buf: &'static mut T,
//     ) -> Option<&'static mut dyn PacketBuffer<CONTIGUOUS, HDR_RSV>> {
//         if self.state.get() {
//             // Already captured a type
//             None
//         } else {
//             self.state.set(true);
//             Some(buf)
//         }
//     }

//     pub fn restore(
//         &self,
//         dyn_buf: &'static mut dyn PacketBuffer<CONTIGUOUS, HDR_RSV>,
//     ) -> Option<&'static mut T> {
//         // TODO: how can we make sure that this buffer is of an
//         // identical type to the one we passed down previously?
//         if self.state.get() {
//             Some(unsafe {
//                 std::mem::transmute::<*mut (), &'static mut T>(
//                     dyn_buf as *mut dyn PacketBuffer<CONTIGUOUS, HDR_RSV> as *mut (),
//                 )
//             })
//         } else {
//             None
//         }
//     }
// }

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::Cell;

    //     #[test]
    //     fn test_types() {
    //         // fn send<'a, PB: PacketBuffer<true, 12> + 'a>(buffer: PB) {}
    //         fn accept_dyn_pb(_pb: &mut dyn PacketBuffer<true, 0>) {}

    //         let empty_pb = Box::leak(Box::new(PacketBufferEnd::new()));

    //         let mut packet_data_arr = Box::leak(Box::new([0_u8; 1500]));
    //         let mut hdr_mut_pb = PacketSlice::<true, 32, true, 0, _, _>::from_slice_mut_end(
    //             packet_data_arr,
    //         );

    //         let hdr_mut_pb_resized: &mut dyn PacketBuffer<true, 16> =
    //             shrink_packet_buffer_headroom(&mut hdr_mut_pb);

    //         // let arr_next_pb: PacketArr<false, 12, true, 16, &dyn PacketBuffer<true, 16>> =
    //         //     PacketArr::from_arr([0; 12], hdr_mut_pb_resized);
    //     }

    struct ImANetworkLayer<'a, const CONTIGUOUS: bool, const HEADROOM: usize> {
        higher_layer_adaptors:
            [Cell<Option<&'a dyn ThisIsAHigherLayerAdaptor<CONTIGUOUS, HEADROOM>>>; 1],
    }

    impl<const CONTIGUOUS: bool, const HEADROOM: usize> ImANetworkLayer<'_, CONTIGUOUS, HEADROOM> {
        fn dispatch_buffer(&self, buffer: PacketBufferMut<CONTIGUOUS, HEADROOM>) {
            self.higher_layer_adaptors[0]
                .get()
                .unwrap()
                .pass_buffer_back(buffer);
        }
    }

    trait ThisIsAHigherLayerAdaptor<const CONTIGUOUS: bool, const HEADROOM: usize> {
        fn pass_buffer_back(&self, buffer: PacketBufferMut<CONTIGUOUS, HEADROOM>);
    }

    struct ImAHigherLayerAdaptor<
        'a,
        const CONTIGUOUS: bool,
        const NETWORK_LAYER_HEADROOM: usize,
        const HIGHER_LAYER_HEADROOM: usize,
    > {
        network_layer: &'a ImANetworkLayer<'a, false, NETWORK_LAYER_HEADROOM>,
        // type_capture: BufferTypeCapture<CONTIGUOUS, HIGHER_LAYER_HEADROOM, HPB>,
    }

    impl<
            'a,
            const CONTIGUOUS: bool,
            const NETWORK_LAYER_HEADROOM: usize,
            const HIGHER_LAYER_HEADROOM: usize,
            // HPB: PacketBuffer + 'static,
        > ThisIsAHigherLayerAdaptor<CONTIGUOUS, NETWORK_LAYER_HEADROOM>
        for ImAHigherLayerAdaptor<'a, CONTIGUOUS, NETWORK_LAYER_HEADROOM, HIGHER_LAYER_HEADROOM>
    {
        fn pass_buffer_back(&self, buffer: PacketBufferMut<CONTIGUOUS, NETWORK_LAYER_HEADROOM>) {
            let restored: PacketBufferMut<CONTIGUOUS, HIGHER_LAYER_HEADROOM> = buffer
                .restore_headroom()
                .expect("Buffer is smaller than expected!");
        }
    }

    impl<'a, const NETWORK_LAYER_HEADROOM: usize, const HIGHER_LAYER_HEADROOM: usize>
        ImAHigherLayerAdaptor<'a, true, NETWORK_LAYER_HEADROOM, HIGHER_LAYER_HEADROOM>
    {
        fn dispatch_buffer<const HIGHER_LAYER_CONTIGUOUS: bool>(
            &self,
            buf: PacketBufferMut<HIGHER_LAYER_CONTIGUOUS, HIGHER_LAYER_HEADROOM>,
        ) {
            // We get some buffer, want to prepend a header, and then pass it on
            // to the next. For code in Tock, this will require some form of
            // allocation pool at the respective layers (or sufficient headroom
            // in the original buffer), but here we simply decide to leak
            // memory.
            let header_arr = Box::leak(Box::new(PacketArr::from_arr(
                [0, 1, 2, 3],
                buf.into_dyn_mut(),
            )));

            let pb: PacketBufferMut<false, 0> = PacketBufferMut::from_mut_arr(header_arr);

            self.network_layer.dispatch_buffer(pb.reduce_headroom());
        }
    }

    // struct ImAHigherLayerAdaptor2<
    //     'a,
    //     const CONTIGUOUS: bool,
    //     const NETWORK_LAYER_HEADROOM: usize,
    //     const HIGHER_LAYER_HEADROOM: usize,
    // > {
    //     network_layer: &'a ImANetworkLayer<'a, CONTIGUOUS, NETWORK_LAYER_HEADROOM>,
    //     // type_capture: BufferTypeCapture<CONTIGUOUS, HIGHER_LAYER_HEADROOM, HPB>,
    // }

    // impl<
    //         'a,
    //         const HIGHER_LAYER_CONTIGUOUS: bool,
    //         const NETWORK_LAYER_HEADROOM: usize,
    //         const HIGHER_LAYER_HEADROOM: usize,
    //     > ThisIsAHigherLayerAdaptor<CONTIGUOUS, NETWORK_LAYER_HEADROOM>
    //     for ImAHigherLayerAdaptor2<'a, CONTIGUOUS, NETWORK_LAYER_HEADROOM, HIGHER_LAYER_HEADROOM>
    // {
    //     fn pass_buffer_back(&self, buffer: PacketBufferMut<CONTIGUOUS, NETWORK_LAYER_HEADROOM>) {
    //         let restored: PacketBufferMut<CONTIGUOUS, HIGHER_LAYER_HEADROOM> = buffer
    //             .restore_headroom()
    //             .expect("Buffer is smaller than expected!");
    //     }
    // }

    // impl<'a, const NETWORK_LAYER_HEADROOM: usize, const HIGHER_LAYER_HEADROOM: usize>
    //     ImAHigherLayerAdaptor2<'a, true, NETWORK_LAYER_HEADROOM, HIGHER_LAYER_HEADROOM>
    // {
    //     fn dispatch_buffer<const HIGHER_LAYER_CONTIGUOUS: bool>(
    //         &self,
    //         buf: PacketBufferMut<HIGHER_LAYER_CONTIGUOUS, HIGHER_LAYER_HEADROOM>,
    //     ) {
    //         // We get some buffer, want to prepend a header, and then pass it on
    //         // to the next. For code in Tock, this will require some form of
    //         // allocation pool at the respective layers (or sufficient headroom
    //         // in the original buffer), but here we simply decide to leak
    //         // memory.
    //         let header_arr = Box::leak(Box::new(PacketArr::from_arr(
    //             [0, 1, 2, 3],
    //             buf.into_dyn_mut(),
    //         )));

    //         let pb = PacketBufferMut::from_mut_arr(header_arr);

    //         self.network_layer.dispatch_buffer(pb.reduce_headroom());
    //     }
    // }

    //     #[test]
    //     fn test_layers() {
    // 	let mut network_layer = ImANetworkLayer {
    // 	    higher_layer_adaptors: [Cell::new(None)],
    // 	};

    // 	let adaptor: ImAHigherLayerAdaptor<'_, true, 0, 0, PacketBufferEnd> = ImAHigherLayerAdaptor {
    // 	    network_layer: &network_layer,
    // 	    _pd: PhantomData,
    // 	};

    // 	network_layer.higher_layer_adaptors[0].set(Some(&adaptor));

    // 	adaptor.dispatch_buffer(Box::leak(Box::new(PacketBufferEnd)));
    //     }
}
