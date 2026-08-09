//! Bounded decoders for image payloads embedded in RTF.

pub(super) mod dib;
pub(super) mod jpeg;
pub(super) mod png;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(in crate::rtf::parser) enum MediaDecodeError {
    Malformed,
    PixelLimitExceeded { width_px: u32, height_px: u32 },
}
