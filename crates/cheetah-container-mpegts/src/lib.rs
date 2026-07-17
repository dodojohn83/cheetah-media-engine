//! MPEG-TS transport stream parser, demuxer, and clock recovery.

#![cfg_attr(not(any(test, feature = "std")), no_std)]
extern crate alloc;

pub mod clock;
pub mod demuxer;
pub mod error;
pub mod packet;
pub mod pes;
pub mod section;

pub use clock::{ClockState, PcrClock};
pub use demuxer::{TsDemuxer, TsDiagnostics, TsEvent};
pub use error::TsError;
pub use packet::TsPacket;
pub use pes::{PesAssembler, PesHeader, PesOutput};
pub use section::{PatEntry, SectionAssembler, parse_pat, parse_pmt};

#[cfg(test)]
mod tests;
