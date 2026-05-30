// Licensed under the Apache License, Version 2.0 or the MIT License.
// SPDX-License-Identifier: Apache-2.0 OR MIT
// Copyright (c) 2024 Antmicro <www.antmicro.com>

use core::cell::Cell;
use core::ptr::write_volatile;
use kernel::deferred_call::{DeferredCall, DeferredCallClient};
use kernel::hil;
use kernel::utilities::cells::{OptionalCell, TakeCell};
use kernel::utilities::packet_buffer::{PacketBufferMut, PacketSliceMut};
use kernel::ErrorCode;

pub struct SemihostUart<'a, const HEAD: usize, const TAIL: usize> {
    deferred_call: DeferredCall,
    tx_client: OptionalCell<&'a dyn hil::uart::TransmitClient<HEAD, TAIL>>,
    tx_buffer: TakeCell<'static, PacketSliceMut>,
    tx_len: Cell<usize>,
}

impl<'a, const HEAD: usize, const TAIL: usize> SemihostUart<'a, HEAD, TAIL> {
    pub fn new() -> SemihostUart<'a, HEAD, TAIL> {
        SemihostUart {
            deferred_call: DeferredCall::new(),
            tx_client: OptionalCell::empty(),
            tx_buffer: TakeCell::empty(),
            tx_len: Cell::new(0),
        }
    }
}

impl<const HEAD: usize, const TAIL: usize> Default for SemihostUart<'_, HEAD, TAIL> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const HEAD: usize, const TAIL: usize> hil::uart::Configure for SemihostUart<'_, HEAD, TAIL> {
    fn configure(&self, _params: hil::uart::Parameters) -> Result<(), ErrorCode> {
        Ok(())
    }
}

impl<'a, const HEAD: usize, const TAIL: usize> hil::uart::Transmit<'a, HEAD, TAIL>
    for SemihostUart<'a, HEAD, TAIL>
{
    fn set_transmit_client(&self, client: &'a dyn hil::uart::TransmitClient<HEAD, TAIL>) {
        self.tx_client.set(client);
    }

    fn transmit_buffer(
        &self,
        tx_buffer: PacketBufferMut<HEAD, TAIL>,
        tx_len: usize,
    ) -> Result<(), (ErrorCode, PacketBufferMut<HEAD, TAIL>)> {
        if tx_len == 0 || tx_len > tx_buffer.len() {
            Err((ErrorCode::SIZE, tx_buffer))
        } else if self.tx_buffer.is_some() {
            Err((ErrorCode::BUSY, tx_buffer))
        } else {
            for b in &tx_buffer.payload()[..tx_len] {
                unsafe {
                    // Print to this address for simulation output
                    write_volatile(0xd0580000 as *mut u32, (*b) as u32);
                }
            }
            self.tx_len.set(tx_len);
            self.tx_buffer.replace(tx_buffer.downcast().unwrap());
            // The whole buffer was transmited immediately
            self.deferred_call.set();
            Ok(())
        }
    }

    fn transmit_word(&self, _word: u32) -> Result<(), ErrorCode> {
        Err(ErrorCode::FAIL)
    }

    fn transmit_abort(&self) -> Result<(), ErrorCode> {
        Err(ErrorCode::FAIL)
    }
}

impl<'a, const HEAD: usize, const TAIL: usize> hil::uart::Receive<'a>
    for SemihostUart<'a, HEAD, TAIL>
{
    fn set_receive_client(&self, _client: &'a dyn hil::uart::ReceiveClient) {}
    fn receive_buffer(
        &self,
        rx_buffer: &'static mut [u8],
        _rx_len: usize,
    ) -> Result<(), (ErrorCode, &'static mut [u8])> {
        Err((ErrorCode::FAIL, rx_buffer))
    }
    fn receive_word(&self) -> Result<(), ErrorCode> {
        Err(ErrorCode::FAIL)
    }
    fn receive_abort(&self) -> Result<(), ErrorCode> {
        Err(ErrorCode::FAIL)
    }
}

impl<const HEAD: usize, const TAIL: usize> DeferredCallClient for SemihostUart<'_, HEAD, TAIL> {
    fn register(&'static self) {
        self.deferred_call.register(self);
    }

    fn handle_deferred_call(&self) {
        self.tx_client.map(|client| {
            self.tx_buffer.take().map(|tx_buf| {
                client.transmitted_buffer(
                    PacketBufferMut::new(tx_buf).unwrap(),
                    self.tx_len.get(),
                    Ok(()),
                );
            });
        });
    }
}
