//! Variant selection for HLS master playlists.

use alloc::string::String;
use alloc::vec::Vec;

use crate::error::HlsError;
use crate::model::Variant;

/// Capabilities used to filter variants.
#[derive(Debug, Clone, Default)]
pub struct VariantCapabilities {
    pub max_bandwidth: Option<u32>,
    pub min_bandwidth: Option<u32>,
    pub max_width: Option<u32>,
    pub max_height: Option<u32>,
    /// Required codec prefixes, e.g. `avc1`, `mp4a`.
    pub required_codecs: Vec<String>,
    pub allow_video: bool,
    pub allow_audio: bool,
}

impl VariantCapabilities {
    pub fn can_decode(&self, variant: &Variant) -> bool {
        codec_compatible(variant, self)
    }
}

/// Trait for selecting a variant.
pub trait VariantSelector: core::fmt::Debug {
    fn select<'a>(
        &self,
        variants: &'a [Variant],
        caps: &VariantCapabilities,
    ) -> Result<&'a Variant, HlsError>;
}

/// Default bandwidth-based selector.
#[derive(Debug, Clone, Default)]
pub struct BandwidthSelector;

impl VariantSelector for BandwidthSelector {
    fn select<'a>(
        &self,
        variants: &'a [Variant],
        caps: &VariantCapabilities,
    ) -> Result<&'a Variant, HlsError> {
        select_initial_variant(variants, caps)
    }
}

/// Select the highest-bandwidth variant that satisfies `caps`.
///
/// When no precise match exists, falls back to the lowest bandwidth variant
/// that is still decodable, avoiding variants that exceed hard limits.
pub fn select_initial_variant<'a>(
    variants: &'a [Variant],
    caps: &VariantCapabilities,
) -> Result<&'a Variant, HlsError> {
    if variants.is_empty() {
        return Err(HlsError::missing_tag("variant"));
    }

    let mut candidates: Vec<&Variant> = variants
        .iter()
        .filter(|v| codec_compatible(v, caps) && within_limits(v, caps))
        .collect();

    if candidates.is_empty() {
        // Relax bandwidth limits but keep decodability.
        candidates = variants
            .iter()
            .filter(|v| codec_compatible(v, caps))
            .collect();
    }

    if candidates.is_empty() {
        return Err(HlsError::Unsupported {
            feature: "no decodable variant".into(),
        });
    }

    // Prefer highest bandwidth under max_bandwidth if set; otherwise highest overall.
    candidates.sort_by(|a, b| {
        let a_bw = a.average_bandwidth.unwrap_or(a.bandwidth);
        let b_bw = b.average_bandwidth.unwrap_or(b.bandwidth);
        if let Some(max) = caps.max_bandwidth {
            let a_fit = a_bw <= max;
            let b_fit = b_bw <= max;
            match (a_fit, b_fit) {
                (true, false) => core::cmp::Ordering::Greater,
                (false, true) => core::cmp::Ordering::Less,
                _ => a_bw.cmp(&b_bw),
            }
        } else {
            a_bw.cmp(&b_bw)
        }
    });

    Ok(*candidates.last().expect("non-empty candidates"))
}

fn codec_compatible(variant: &Variant, caps: &VariantCapabilities) -> bool {
    if caps.required_codecs.is_empty() {
        return true;
    }
    if variant.codecs.is_empty() {
        // Without codec info we cannot confirm compatibility.
        return false;
    }
    caps.required_codecs
        .iter()
        .any(|req| variant.codecs.iter().any(|c| c.starts_with(req.as_str())))
}

fn within_limits(variant: &Variant, caps: &VariantCapabilities) -> bool {
    if let Some(min) = caps.min_bandwidth
        && variant.bandwidth < min
    {
        return false;
    }
    if let Some(max) = caps.max_bandwidth
        && variant.bandwidth > max
    {
        return false;
    }
    if let (Some(max_w), Some((w, _))) = (caps.max_width, variant.resolution)
        && w > max_w
    {
        return false;
    }
    if let (Some(max_h), Some((_, h))) = (caps.max_height, variant.resolution)
        && h > max_h
    {
        return false;
    }
    true
}
