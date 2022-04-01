use core::fmt::Write;
use core::fmt::Debug;

use crate::{virtmem::KernPointer, X86Default};
use packed_struct::prelude::*;



pub struct UARTDevice {
    data: KernPointer<u8>,
    int_en: KernPointer<u8>,
    fifo_ctrl: KernPointer<u8>,
    line_ctrl: KernPointer<u8>,
    modem_ctrl: KernPointer<u8>,
    line_status: KernPointer<u8>,
}

#[derive(PackedStruct, Default)]
#[packed_struct(bit_numbering = "lsb0", size_bytes = "1")]
struct LineStatusFlags {
    #[packed_field(bits = "0")]
    input_full: bool,
    #[packed_field(bits = "1")]
    overrun_error: bool,
    #[packed_field(bits = "2")]
    parity_error: bool,
    #[packed_field(bits = "3")]
    framing_error: bool,
    #[packed_field(bits = "4")]
    brk_signal: bool,
    #[packed_field(bits = "5")]
    output_empty: bool,
    #[packed_field(bits = "6")]
    output_empty_and_line_idle: bool,
    #[packed_field(bits = "7")]
    bad_data: bool,
}

#[derive(PackedStruct, Default)]
#[packed_struct(bit_numbering = "lsb0", size_bytes = "1")]
struct LineControlRegister {
    #[packed_field(bits = "0..1")]
    no_of_data_bits: u8,
    #[packed_field(bits = "2")]
    has_stop_bit: bool,
    #[packed_field(bits = "3")]
    has_parity: bool,
    #[packed_field(bits = "4..5")]
    parity_type: bool,
    #[packed_field(bits = "6")]
    brk_signal_disabled: bool,
    #[packed_field(bits = "7")]
    divisor_latch_access_bit: bool,
}

#[derive(PackedStruct, Default)]
#[packed_struct(bit_numbering = "lsb0", size_bytes = "1")]
struct InterruptEnableRegister {
    #[packed_field(bits = "0")]
    data_available_interrupt: bool,
    #[packed_field(bits = "1")]
    transmitter_holding_register_empty_interrupt: bool,
    #[packed_field(bits = "2")]
    line_status_register_change_interrupt: bool,
    #[packed_field(bits = "3")]
    modem_status_register_change_interrupt: bool,
}

impl InterruptEnableRegister {
    pub fn all_disabled() -> Self {
        Self::default()
    }
}

#[derive(PackedStruct, Default)]
#[packed_struct(bit_numbering = "lsb0", size_bytes = "1")]
struct ModemControlRegister {
    #[packed_field(bits = "0")]
    uart_ready: bool,
    #[packed_field(bits = "1")]
    request_to_send: bool,
    #[packed_field(bits = "2")]
    aux_output1: bool,
    #[packed_field(bits = "3")]
    aux_output2: bool,
    #[packed_field(bits = "4")]
    loopback_mode: bool,
}

#[derive(PackedStruct, Default)]
#[packed_struct(bit_numbering = "lsb0", size_bytes = "1")]
struct FIFOControlRegister {
    #[packed_field(bits = "0")]
    fifo_enable: bool,
    #[packed_field(bits = "1")]
    clear_recive_fifo: bool,
    #[packed_field(bits = "2")]
    clear_transmit_fifo: bool,
    #[packed_field(bits = "3")]
    dma_mode: bool,
    #[packed_field(bits = "7..6")]
    fifo_size: u8,
}

impl UARTDevice {
    pub unsafe fn new(base: KernPointer<u8>) -> Self {
        Self {
            data: base,
            int_en: base.offset(1),
            fifo_ctrl: base.offset(2),
            line_ctrl: base.offset(3),
            modem_ctrl: base.offset(4),
            line_status: base.offset(5),
        }
    }

    pub fn init(&mut self) {
        unsafe {
            // Disable interrupts
            self.int_en.write(0x00);

            // Set divisor for baud rate
            /***************************************/
            // Enable setting the divisor
            self.line_ctrl.write(
                LineControlRegister {
                    divisor_latch_access_bit: true,
                    ..LineControlRegister::default()
                }
                .pack()
                .unwrap()[0],
            );
            // Set 38400=115200/3 baud rate
            self.data.write(0xff);
            self.int_en
                .write(InterruptEnableRegister::all_disabled().pack().unwrap()[0]);

            // Disable setting the divisor and set data length to 8 bits, 1 stop bit, no parity
            self.line_ctrl.write(
                LineControlRegister {
                    no_of_data_bits: 3,
                    ..LineControlRegister::default()
                }
                .pack()
                .unwrap()[0],
            );
            /***************************************/

            
            // Enable FIFO, clear TX/RX queues and set interrupt watermark at 14 bytes
            self.fifo_ctrl.write(
                FIFOControlRegister {
                    fifo_enable: true,
                    clear_recive_fifo: true,
                    clear_transmit_fifo: true,
                    fifo_size: 3,
                    ..FIFOControlRegister::default()
                }
                .pack()
                .unwrap()[0],
            );

            // Mark data terminal ready, signal request to send
            self.modem_ctrl.write(
                ModemControlRegister {
                    uart_ready: true,
                    request_to_send: true,
                    aux_output2: true,
                    ..ModemControlRegister::default()
                }
                .pack()
                .unwrap()[0],
            );

            // We don't enable interrupts for now because we don't use them for now
            // self.int_en.write(InterruptEnableRegister{data_available_interrupt: true, ..InterruptEnableRegister::default()}.pack().unwrap()[0]);
        }
    }
    fn line_sts(&self) -> LineStatusFlags { unsafe { LineStatusFlags::unpack(&[self.line_status.read()]).unwrap() } }

    pub fn send(&mut self, data: u8) {
        unsafe {
            wait_for!(self.line_sts().output_empty);
            self.data.write(data);
        }
    }

    pub fn receive(&self) -> u8 {
        unsafe {
            wait_for!(self.line_sts().input_full);
            self.data.read()
        }
    }
}

impl Debug for UARTDevice{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("UARTDevice").field("data", &self.data).field("int_en", &self.int_en).field("fifo_ctrl", &self.fifo_ctrl).field("line_ctrl", &self.line_ctrl).field("modem_ctrl", &self.modem_ctrl).field("line_status", &self.line_status).finish()
    }
}

impl Write for UARTDevice{
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        s.chars().for_each(|c|{
            self.send(c as u8);
            if c == '\n' {
                self.send(b'\r')
            }
        });
        Ok(())
    }
}

impl X86Default for UARTDevice {
    unsafe fn x86_default() -> Self { Self::new(KernPointer::<u8>::from_port(0x3f8)) }
}
