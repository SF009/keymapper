//! Wire protocol shared by the Linux host and Android gateway.
//!
//! Header:
//!   magic   u32 BE = 0x57444B4D ("WDKM")
//!   version u8      = 1
//!   type    u8
//!   len     u16 BE
//!
//! Payloads:
//!   KEY          [code:u16 BE, state:u8]
//!   MOUSE_MOVE   [dx:i16 BE, dy:i16 BE]
//!   MOUSE_BUTTON [code:u16 BE, state:u8]
//!
//! The Android side remains backward-compatible with this framing.

#![allow(dead_code)]

pub const MAGIC: u32 = 0x5744_4B4D;
pub const VERSION: u8 = 1;
pub const HELLO: u8 = 1;
pub const KEY: u8 = 2;
pub const MOUSE_MOVE: u8 = 3;
pub const MOUSE_BUTTON: u8 = 4;
pub const PING: u8 = 5;
