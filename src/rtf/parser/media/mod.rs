//! Bounded decoders for image payloads embedded in RTF.

pub(super) mod dib;
pub(super) mod emf;
pub(super) mod jpeg;
pub(super) mod png;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(in crate::rtf::parser) enum MediaDecodeError {
    Malformed,
    PixelLimitExceeded { width_px: u32, height_px: u32 },
}

#[derive(Debug, Copy, Clone)]
pub(in crate::rtf::parser) struct VectorDecodeLimits {
    pub max_points_per_record: usize,
    pub max_raster_pixels: usize,
}
pub(super) mod wmf;
