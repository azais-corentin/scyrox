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
// DpiStage Conversions
// =============================================================================

impl From<&scyrox::DpiStage> for crate::DpiStage {
    fn from(stage: &scyrox::DpiStage) -> Self {
        crate::DpiStage {
            value: stage.value as u32,
            red: stage.color[0] as u32,
            green: stage.color[1] as u32,
            blue: stage.color[2] as u32,
        }
    }
}

impl TryFrom<&crate::DpiStage> for scyrox::DpiStage {
    type Error = ConversionError;

    fn try_from(stage: &crate::DpiStage) -> Result<Self, Self::Error> {
        let value = u16::try_from(stage.value)
            .map_err(|_| ConversionError::InvalidDpiValue(stage.value))?;
        let component =
            |c: u32| u8::try_from(c).map_err(|_| ConversionError::InvalidDpiColorComponent(c));
        Ok(scyrox::DpiStage {
            value,
            color: [
                component(stage.red)?,
                component(stage.green)?,
                component(stage.blue)?,
            ],
        })
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
            dpi_stages: config
                .dpi_stages
                .iter()
                .map(crate::DpiStage::from)
                .collect(),
            current_dpi_index: config.current_dpi_index as u32,
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
            dpi_stages: proto
                .dpi_stages
                .iter()
                .map(scyrox::DpiStage::try_from)
                .collect::<Result<_, _>>()?,
            current_dpi_index: u8::try_from(proto.current_dpi_index)
                .map_err(|_| ConversionError::InvalidDpiIndex(proto.current_dpi_index))?,
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
    /// Invalid DPI value (exceeds u16 range).
    InvalidDpiValue(u32),
    /// Invalid DPI color component (exceeds u8 range).
    InvalidDpiColorComponent(u32),
    /// Invalid DPI index (exceeds u8 range).
    InvalidDpiIndex(u32),
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
            ConversionError::InvalidDpiValue(v) => {
                write!(f, "invalid DPI value: {}", v)
            }
            ConversionError::InvalidDpiColorComponent(v) => {
                write!(f, "invalid DPI color component: {}", v)
            }
            ConversionError::InvalidDpiIndex(v) => {
                write!(f, "invalid DPI index: {}", v)
            }
        }
    }
}

impl std::error::Error for ConversionError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dpi_stage_roundtrip() {
        let domain = scyrox::DpiStage {
            value: 1600,
            color: [255, 128, 0],
        };
        let proto = crate::DpiStage::from(&domain);
        assert_eq!(proto.value, 1600);
        assert_eq!((proto.red, proto.green, proto.blue), (255, 128, 0));
        assert_eq!(scyrox::DpiStage::try_from(&proto).unwrap(), domain);
    }

    #[test]
    fn dpi_stage_rejects_out_of_range() {
        let bad_value = crate::DpiStage {
            value: 70_000,
            red: 0,
            green: 0,
            blue: 0,
        };
        assert_eq!(
            scyrox::DpiStage::try_from(&bad_value),
            Err(ConversionError::InvalidDpiValue(70_000))
        );
        let bad_color = crate::DpiStage {
            value: 800,
            red: 300,
            green: 0,
            blue: 0,
        };
        assert_eq!(
            scyrox::DpiStage::try_from(&bad_color),
            Err(ConversionError::InvalidDpiColorComponent(300))
        );
    }

    #[test]
    fn mouse_config_dpi_roundtrip() {
        let domain = scyrox::MouseConfig {
            dpi_stages: vec![
                scyrox::DpiStage {
                    value: 800,
                    color: [255, 0, 0],
                },
                scyrox::DpiStage {
                    value: 1600,
                    color: [255, 255, 255],
                },
            ],
            current_dpi_index: 1,
            ..Default::default()
        };
        let proto = ProtoConfig::from(&domain);
        assert_eq!(proto.dpi_stages.len(), 2);
        assert_eq!(proto.current_dpi_index, 1);
        let back = scyrox::MouseConfig::try_from(&proto).unwrap();
        assert_eq!(back.dpi_stages, domain.dpi_stages);
        assert_eq!(back.current_dpi_index, 1);
    }

    #[test]
    fn mouse_config_rejects_bad_dpi_index() {
        let mut proto = ProtoConfig::from(&scyrox::MouseConfig::default());
        proto.current_dpi_index = 999;
        assert_eq!(
            scyrox::MouseConfig::try_from(&proto).unwrap_err(),
            ConversionError::InvalidDpiIndex(999)
        );
    }
}
