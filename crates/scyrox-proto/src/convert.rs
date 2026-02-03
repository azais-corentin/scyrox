//! Conversion utilities between proto types and scyrox library types.
//!
//! This module provides `From` trait implementations for converting between
//! protobuf-generated types and the corresponding scyrox library types.

use crate::{LiftOffDistance as ProtoLod, MouseConfig as ProtoConfig, PollingRate as ProtoRate};

// =============================================================================
// PollingRate Conversions
// =============================================================================

impl From<scyrox::PollingRate> for ProtoRate {
    fn from(rate: scyrox::PollingRate) -> Self {
        match rate {
            scyrox::PollingRate::Hz125 => ProtoRate::PollingRate125,
            scyrox::PollingRate::Hz250 => ProtoRate::PollingRate250,
            scyrox::PollingRate::Hz500 => ProtoRate::PollingRate500,
            scyrox::PollingRate::Hz1000 => ProtoRate::PollingRate1000,
            scyrox::PollingRate::Hz2000 => ProtoRate::PollingRate2000,
            scyrox::PollingRate::Hz4000 => ProtoRate::PollingRate4000,
            scyrox::PollingRate::Hz8000 => ProtoRate::PollingRate8000,
        }
    }
}

impl TryFrom<ProtoRate> for scyrox::PollingRate {
    type Error = ConversionError;

    fn try_from(rate: ProtoRate) -> Result<Self, Self::Error> {
        match rate {
            ProtoRate::Unspecified => Err(ConversionError::UnspecifiedPollingRate),
            ProtoRate::PollingRate125 => Ok(scyrox::PollingRate::Hz125),
            ProtoRate::PollingRate250 => Ok(scyrox::PollingRate::Hz250),
            ProtoRate::PollingRate500 => Ok(scyrox::PollingRate::Hz500),
            ProtoRate::PollingRate1000 => Ok(scyrox::PollingRate::Hz1000),
            ProtoRate::PollingRate2000 => Ok(scyrox::PollingRate::Hz2000),
            ProtoRate::PollingRate4000 => Ok(scyrox::PollingRate::Hz4000),
            ProtoRate::PollingRate8000 => Ok(scyrox::PollingRate::Hz8000),
        }
    }
}

// =============================================================================
// LiftOffDistance Conversions
// =============================================================================

impl From<scyrox::LiftOffDistance> for ProtoLod {
    fn from(lod: scyrox::LiftOffDistance) -> Self {
        match lod {
            scyrox::LiftOffDistance::Low => ProtoLod::Low,
            scyrox::LiftOffDistance::Medium => ProtoLod::Medium,
            scyrox::LiftOffDistance::High => ProtoLod::High,
        }
    }
}

impl TryFrom<ProtoLod> for scyrox::LiftOffDistance {
    type Error = ConversionError;

    fn try_from(lod: ProtoLod) -> Result<Self, Self::Error> {
        match lod {
            ProtoLod::Unspecified => Err(ConversionError::UnspecifiedLiftOffDistance),
            ProtoLod::Low => Ok(scyrox::LiftOffDistance::Low),
            ProtoLod::Medium => Ok(scyrox::LiftOffDistance::Medium),
            ProtoLod::High => Ok(scyrox::LiftOffDistance::High),
        }
    }
}

// =============================================================================
// MouseConfig Conversions
// =============================================================================

impl From<&scyrox::MouseConfig> for ProtoConfig {
    fn from(config: &scyrox::MouseConfig) -> Self {
        ProtoConfig {
            polling_rate: ProtoRate::from(config.polling_rate).into(),
            lift_off_distance: ProtoLod::from(config.lift_off_distance).into(),
            sleep_timeout_seconds: config.sleep_timeout_seconds as u32,
            angle_snapping: config.angle_snapping,
            ripple_control: config.ripple_control,
            high_speed_mode: config.high_speed_mode,
            long_distance_mode: config.long_distance_mode,
        }
    }
}

impl From<scyrox::MouseConfig> for ProtoConfig {
    fn from(config: scyrox::MouseConfig) -> Self {
        ProtoConfig::from(&config)
    }
}

impl TryFrom<&ProtoConfig> for scyrox::MouseConfig {
    type Error = ConversionError;

    fn try_from(proto: &ProtoConfig) -> Result<Self, Self::Error> {
        let polling_rate = ProtoRate::try_from(proto.polling_rate)
            .map_err(|_| ConversionError::UnspecifiedPollingRate)?;
        let lift_off_distance = ProtoLod::try_from(proto.lift_off_distance)
            .map_err(|_| ConversionError::UnspecifiedLiftOffDistance)?;

        Ok(scyrox::MouseConfig {
            polling_rate: polling_rate.try_into()?,
            lift_off_distance: lift_off_distance.try_into()?,
            sleep_timeout_seconds: proto.sleep_timeout_seconds as u16,
            angle_snapping: proto.angle_snapping,
            ripple_control: proto.ripple_control,
            high_speed_mode: proto.high_speed_mode,
            long_distance_mode: proto.long_distance_mode,
            ..Default::default()
        })
    }
}

impl TryFrom<ProtoConfig> for scyrox::MouseConfig {
    type Error = ConversionError;

    fn try_from(proto: ProtoConfig) -> Result<Self, Self::Error> {
        scyrox::MouseConfig::try_from(&proto)
    }
}

// =============================================================================
// Helper functions for Hz/mm conversions
// =============================================================================

/// Convert Hz value to proto PollingRate.
pub fn hz_to_proto_polling_rate(hz: u16) -> ProtoRate {
    scyrox::PollingRate::from_hz(hz)
        .map(ProtoRate::from)
        .unwrap_or(ProtoRate::Unspecified)
}

/// Convert mm value to proto LiftOffDistance.
pub fn mm_to_proto_lod(mm: f32) -> ProtoLod {
    scyrox::LiftOffDistance::from_mm(mm)
        .map(ProtoLod::from)
        .unwrap_or_else(|| {
            // Fall back to range-based matching for approximate values
            if mm <= 0.85 {
                ProtoLod::Low
            } else if mm <= 1.5 {
                ProtoLod::Medium
            } else {
                ProtoLod::High
            }
        })
}

// =============================================================================
// Error Types
// =============================================================================

/// Error type for proto ↔ library type conversions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConversionError {
    /// Polling rate was unspecified in the proto message.
    UnspecifiedPollingRate,
    /// Lift-off distance was unspecified in the proto message.
    UnspecifiedLiftOffDistance,
    /// Invalid polling rate value.
    InvalidPollingRate(i32),
    /// Invalid lift-off distance value.
    InvalidLiftOffDistance(i32),
}

impl std::fmt::Display for ConversionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConversionError::UnspecifiedPollingRate => {
                write!(f, "polling rate not specified")
            }
            ConversionError::UnspecifiedLiftOffDistance => {
                write!(f, "lift-off distance not specified")
            }
            ConversionError::InvalidPollingRate(v) => {
                write!(f, "invalid polling rate value: {}", v)
            }
            ConversionError::InvalidLiftOffDistance(v) => {
                write!(f, "invalid lift-off distance value: {}", v)
            }
        }
    }
}

impl std::error::Error for ConversionError {}
