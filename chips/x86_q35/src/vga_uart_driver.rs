// Licensed under the Apache License, Version 2.0 or the MIT License.
// SPDX-License-Identifier: Apache-2.0 OR MIT
// Copyright Tock Contributors 2025.

//! VgaUart` — a **synchronous, write-only** façade that lets capsules
//! use a `hil::uart::Uart`-style interface while we actually write to
//! the VGA text buffer, not a serial port.
//!
//! ## Key design points
//!
//! - Implements the *minimum subset* of `Transmit` required by `MuxUart`.
//!   All writes copy bytes to the global `VGA_TEXT` then schedule a deferred
//!   call to invoke the transmit callback (split-phase contract).
//! - **Receive / abort / re-configure** operations just return
//!   `ErrorCode::NOSUPPORT` — VGA is output-only.

use crate::vga::Vga;
use core::{cell::Cell, cmp};
use kernel::deferred_call::{DeferredCall, DeferredCallClient};
use kernel::hil::uart::{Configure, Parameters, Receive, ReceiveClient, Transmit, TransmitClient};
use kernel::utilities::cells::TakeCell;
use kernel::utilities::packet_buffer::{PacketBufferMut, PacketSliceMut};
use kernel::ErrorCode;
use tock_cells::optional_cell::OptionalCell;

/// UART-compatible wrapper around the VGA text writer.
pub struct VgaText<'a, const HEAD: usize, const TAIL: usize> {
    vga_buffer: Vga,
    tx_client: OptionalCell<&'a dyn TransmitClient<HEAD, TAIL>>,
    rx_client: OptionalCell<&'a dyn ReceiveClient>,
    deferred_call: DeferredCall,
    pending_buf: TakeCell<'static, PacketSliceMut>,
    pending_len: Cell<usize>,
}

impl<const HEAD: usize, const TAIL: usize> VgaText<'_, HEAD, TAIL> {
    pub fn new() -> Self {
        Self {
            vga_buffer: Vga::new(),
            tx_client: OptionalCell::empty(),
            rx_client: OptionalCell::empty(),
            deferred_call: DeferredCall::new(),
            pending_buf: TakeCell::empty(),
            pending_len: Cell::new(0),
        }
    }

    fn fire_tx_callback(&self, buf: PacketBufferMut<HEAD, TAIL>, len: usize) {
        self.tx_client.map(|client| {
            client.transmitted_buffer(buf, len, Ok(()));
        });
    }
}

// DeferredCallClient implementation
impl<const HEAD: usize, const TAIL: usize> DeferredCallClient for VgaText<'_, HEAD, TAIL> {
    fn handle_deferred_call(&self) {
        if let Some(buf) = self.pending_buf.take() {
            let len = self.pending_len.get();
            self.fire_tx_callback(PacketBufferMut::new(buf).unwrap(), len);
        }
    }

    fn register(&'static self) {
        self.deferred_call.register(self);
    }
}

// Transmit for Vga
impl<'a, const HEAD: usize, const TAIL: usize> Transmit<'a, HEAD, TAIL>
    for VgaText<'a, HEAD, TAIL>
{
    fn set_transmit_client(&self, client: &'a dyn TransmitClient<HEAD, TAIL>) {
        self.tx_client.set(client);
    }

    fn transmit_buffer(
        &self,
        buffer: PacketBufferMut<HEAD, TAIL>,
        len: usize,
    ) -> Result<(), (ErrorCode, PacketBufferMut<HEAD, TAIL>)> {
        let write_len = cmp::min(len, buffer.payload().len());
        for &byte in &buffer.payload()[..write_len] {
            self.vga_buffer.write_byte(byte);
        }
        self.pending_buf.replace(buffer.downcast().unwrap());
        self.pending_len.set(len);
        self.deferred_call.set();
        Ok(())
    }

    fn transmit_word(&self, _word: u32) -> Result<(), ErrorCode> {
        Err(ErrorCode::NOSUPPORT)
    }

    fn transmit_abort(&self) -> Result<(), ErrorCode> {
        Err(ErrorCode::NOSUPPORT)
    }
}

// Receive for Vga
impl<'a, const HEAD: usize, const TAIL: usize> Receive<'a> for VgaText<'a, HEAD, TAIL> {
    fn set_receive_client(&self, client: &'a dyn ReceiveClient) {
        self.rx_client.set(client);
    }

    fn receive_buffer(
        &self,
        buffer: &'static mut [u8],
        _len: usize,
    ) -> Result<(), (ErrorCode, &'static mut [u8])> {
        Err((ErrorCode::NOSUPPORT, buffer))
    }

    fn receive_word(&self) -> Result<(), ErrorCode> {
        Err(ErrorCode::NOSUPPORT)
    }

    fn receive_abort(&self) -> Result<(), ErrorCode> {
        Err(ErrorCode::NOSUPPORT)
    }
}

// Configure for Vga
impl<const HEAD: usize, const TAIL: usize> Configure for VgaText<'_, HEAD, TAIL> {
    fn configure(&self, _params: Parameters) -> Result<(), ErrorCode> {
        Ok(())
    }
}
