//! Limited AMF0 / ECMA array parser for FLV `onMetaData`.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::FlvError;

/// A parsed AMF0 value.
#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::derive_partial_eq_without_eq)]
pub enum AmfValue {
    Number(f64),
    Boolean(bool),
    String(String),
    /// ECMA array or strict array values.
    Array(Vec<(String, AmfValue)>),
    Object(Vec<(String, AmfValue)>),
    Null,
    Unsupported,
}

impl AmfValue {
    /// Coerce the value to `f64`, if it is a number or boolean.
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Self::Number(v) => Some(*v),
            Self::Boolean(v) => Some(if *v { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    /// Coerce the value to a string slice.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Look up a property by name in an object / array.
    pub fn get(&self, name: &str) -> Option<&AmfValue> {
        match self {
            Self::Object(props) | Self::Array(props) => props
                .iter()
                .find_map(|(k, v)| if k == name { Some(v) } else { None }),
            _ => None,
        }
    }
}

/// Limits for AMF parsing.
#[derive(Debug, Clone, Copy)]
pub struct AmfLimits {
    pub max_depth: u8,
    pub max_total_bytes: u64,
    pub max_string_len: u32,
    pub max_properties: u32,
}

impl Default for AmfLimits {
    fn default() -> Self {
        Self {
            max_depth: 8,
            max_total_bytes: 64 * 1024, // 64 KiB
            max_string_len: 1024,
            max_properties: 1024,
        }
    }
}

struct AmfParser<'a> {
    data: &'a [u8],
    pos: usize,
    limits: AmfLimits,
    depth: u8,
}

impl<'a> AmfParser<'a> {
    fn new(data: &'a [u8], limits: AmfLimits) -> Self {
        Self {
            data,
            pos: 0,
            limits,
            depth: 0,
        }
    }

    fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    fn ensure(&self, n: usize) -> Result<(), FlvError> {
        if self.remaining() < n {
            return Err(FlvError::NeedMoreData);
        }
        Ok(())
    }

    fn read_u8(&mut self) -> Result<u8, FlvError> {
        self.ensure(1)?;
        let v = self.data[self.pos];
        self.pos += 1;
        Ok(v)
    }

    fn read_u16_be(&mut self) -> Result<u16, FlvError> {
        self.ensure(2)?;
        let v = u16::from_be_bytes([self.data[self.pos], self.data[self.pos + 1]]);
        self.pos += 2;
        Ok(v)
    }

    fn read_u32_be(&mut self) -> Result<u32, FlvError> {
        self.ensure(4)?;
        let v = u32::from_be_bytes([
            self.data[self.pos],
            self.data[self.pos + 1],
            self.data[self.pos + 2],
            self.data[self.pos + 3],
        ]);
        self.pos += 4;
        Ok(v)
    }

    fn read_f64_be(&mut self) -> Result<f64, FlvError> {
        self.ensure(8)?;
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&self.data[self.pos..self.pos + 8]);
        self.pos += 8;
        Ok(f64::from_be_bytes(bytes))
    }

    fn read_string(&mut self) -> Result<String, FlvError> {
        let len = self.read_u16_be()? as usize;
        if len as u32 > self.limits.max_string_len {
            return Err(FlvError::LimitExceeded);
        }
        self.ensure(len)?;
        let bytes = &self.data[self.pos..self.pos + len];
        self.pos += len;
        String::from_utf8(bytes.to_vec()).map_err(|_| FlvError::InvalidAmf)
    }

    fn read_long_string(&mut self) -> Result<String, FlvError> {
        let len = self.read_u32_be()? as usize;
        if len as u32 > self.limits.max_string_len {
            return Err(FlvError::LimitExceeded);
        }
        self.ensure(len)?;
        let bytes = &self.data[self.pos..self.pos + len];
        self.pos += len;
        String::from_utf8(bytes.to_vec()).map_err(|_| FlvError::InvalidAmf)
    }

    fn parse_value(&mut self) -> Result<AmfValue, FlvError> {
        if self.depth > self.limits.max_depth {
            return Err(FlvError::InvalidAmf);
        }
        if self.pos as u64 > self.limits.max_total_bytes {
            return Err(FlvError::LimitExceeded);
        }
        let marker = self.read_u8()?;
        match marker {
            0x00 => Ok(AmfValue::Number(self.read_f64_be()?)),
            0x01 => Ok(AmfValue::Boolean(self.read_u8()? != 0)),
            0x02 => Ok(AmfValue::String(self.read_string()?)),
            0x03 => self.parse_object(),
            0x05 => Ok(AmfValue::Null),
            0x06 => Ok(AmfValue::Unsupported),
            0x08 => self.parse_ecma_array(),
            0x09 => Ok(AmfValue::Unsupported), // Object end marker
            0x0a => self.parse_strict_array(),
            0x0b => Ok(AmfValue::String(self.read_long_string()?)),
            0x0c => Ok(AmfValue::Unsupported), // XML document
            _ => Ok(AmfValue::Unsupported),
        }
    }

    fn parse_object(&mut self) -> Result<AmfValue, FlvError> {
        self.depth += 1;
        let mut props = Vec::new();
        loop {
            if self.remaining() < 3 {
                return Err(FlvError::NeedMoreData);
            }
            // Object-end marker: 0x00 0x00 0x09.
            if self.data[self.pos] == 0x00
                && self.data[self.pos + 1] == 0x00
                && self.data[self.pos + 2] == 0x09
            {
                self.pos += 3;
                break;
            }
            let key = self.read_string()?;
            let value = self.parse_value()?;
            props.push((key, value));
            if props.len() as u32 > self.limits.max_properties {
                return Err(FlvError::LimitExceeded);
            }
        }
        self.depth -= 1;
        Ok(AmfValue::Object(props))
    }

    fn parse_ecma_array(&mut self) -> Result<AmfValue, FlvError> {
        self.depth += 1;
        let count = self.read_u32_be()? as usize;
        let count = count.min(self.limits.max_properties as usize);
        let mut props = Vec::with_capacity(count.min(256));
        for _ in 0..count {
            if self.remaining() < 2 {
                return Err(FlvError::NeedMoreData);
            }
            // ECMA array entries also end with object-end marker 0x00 0x00 0x09.
            if self.data[self.pos] == 0x00 && self.data[self.pos + 1] == 0x00 {
                // Need at least 3 bytes for the marker.
                self.ensure(3)?;
                if self.data[self.pos + 2] == 0x09 {
                    self.pos += 3;
                    break;
                }
            }
            let key = self.read_string()?;
            let value = self.parse_value()?;
            props.push((key, value));
        }
        self.depth -= 1;
        Ok(AmfValue::Array(props))
    }

    fn parse_strict_array(&mut self) -> Result<AmfValue, FlvError> {
        self.depth += 1;
        let count = self.read_u32_be()? as usize;
        let count = count.min(self.limits.max_properties as usize);
        let mut items = Vec::with_capacity(count.min(256));
        for i in 0..count {
            let value = self.parse_value()?;
            items.push((i.to_string(), value));
        }
        self.depth -= 1;
        Ok(AmfValue::Array(items))
    }
}

/// Parsed `onMetaData` data block.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FlvScriptData {
    pub name: String,
    pub duration_ms: Option<u64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub frame_rate: Option<f64>,
    pub video_data_rate: Option<u32>,
    pub audio_data_rate: Option<u32>,
    pub video_codec_id: Option<f64>,
    pub audio_codec_id: Option<f64>,
    pub audio_channels: Option<u8>,
    pub stereo: Option<bool>,
    pub raw: alloc::vec::Vec<(String, AmfValue)>,
}

impl FlvScriptData {
    /// Extract a specific property from `raw`.
    pub fn get(&self, name: &str) -> Option<&AmfValue> {
        self.raw
            .iter()
            .find_map(|(k, v)| if k == name { Some(v) } else { None })
    }
}

/// Parse a script tag body that begins with an AMF string (method name) and a value.
pub fn parse_script_data(data: &[u8], limits: AmfLimits) -> Result<FlvScriptData, FlvError> {
    if data.is_empty() {
        return Err(FlvError::NeedMoreData);
    }
    let mut parser = AmfParser::new(data, limits);
    if data[0] != 0x02 {
        return Err(FlvError::InvalidAmf);
    }
    let name = match parser.parse_value()? {
        AmfValue::String(s) => s,
        _ => return Err(FlvError::InvalidAmf),
    };
    let value = parser.parse_value()?;
    let mut meta = FlvScriptData {
        name,
        ..Default::default()
    };

    match value {
        AmfValue::Array(props) | AmfValue::Object(props) => {
            meta.raw = props.clone();
            for (k, v) in &props {
                match k.as_str() {
                    "duration" => {
                        meta.duration_ms = v.as_number().and_then(|n| {
                            if n.is_finite() && n >= 0.0 {
                                Some((n * 1000.0) as u64)
                            } else {
                                None
                            }
                        })
                    }
                    "width" => {
                        meta.width = v.as_number().and_then(|n| {
                            if n.is_finite() && n >= 0.0 {
                                Some(n as u32)
                            } else {
                                None
                            }
                        })
                    }
                    "height" => {
                        meta.height = v.as_number().and_then(|n| {
                            if n.is_finite() && n >= 0.0 {
                                Some(n as u32)
                            } else {
                                None
                            }
                        })
                    }
                    "framerate" | "videoframerate" => {
                        meta.frame_rate = v.as_number().and_then(|n| {
                            if n.is_finite() && n >= 0.0 {
                                Some(n)
                            } else {
                                None
                            }
                        })
                    }
                    "videodatarate" => {
                        meta.video_data_rate = v.as_number().and_then(|n| {
                            if n.is_finite() && n >= 0.0 {
                                Some(n as u32)
                            } else {
                                None
                            }
                        })
                    }
                    "audiodatarate" => {
                        meta.audio_data_rate = v.as_number().and_then(|n| {
                            if n.is_finite() && n >= 0.0 {
                                Some(n as u32)
                            } else {
                                None
                            }
                        })
                    }
                    "videocodecid" => {
                        meta.video_codec_id = v
                            .as_number()
                            .and_then(|n| if n.is_finite() { Some(n) } else { None })
                    }
                    "audiocodecid" => {
                        meta.audio_codec_id = v
                            .as_number()
                            .and_then(|n| if n.is_finite() { Some(n) } else { None })
                    }
                    "audiochannels" => {
                        meta.audio_channels = v.as_number().and_then(|n| {
                            if n.is_finite() && n >= 0.0 {
                                Some(n as u8)
                            } else {
                                None
                            }
                        })
                    }
                    "stereo" => meta.stereo = v.as_number().map(|n| n != 0.0),
                    _ => {}
                }
            }
        }
        _ => {}
    }

    Ok(meta)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_onmetadata_number(name: &str, value: f64) -> Vec<u8> {
        let mut out = Vec::new();
        // Object / ECMA array key is a plain string (2-byte length + bytes, no type marker).
        let name_bytes = name.as_bytes();
        out.extend_from_slice(&(name_bytes.len() as u16).to_be_bytes());
        out.extend_from_slice(name_bytes);
        // Value is a number.
        out.push(0x00);
        out.extend_from_slice(&value.to_be_bytes());
        out
    }

    fn build_number(value: f64) -> Vec<u8> {
        let mut out = Vec::new();
        out.push(0x00);
        out.extend_from_slice(&value.to_be_bytes());
        out
    }

    #[test]
    fn parse_number_property() {
        let data = build_number(1920.0);
        let mut parser = AmfParser::new(&data, AmfLimits::default());
        let value = parser.parse_value().unwrap();
        assert_eq!(value.as_number(), Some(1920.0));
    }

    #[test]
    fn parse_onmetadata_array() {
        // name string "onMetaData" followed by ECMA array with width/height.
        let mut data = Vec::new();
        data.push(0x02);
        let name = b"onMetaData";
        data.extend_from_slice(&(name.len() as u16).to_be_bytes());
        data.extend_from_slice(name);

        // ECMA array
        data.push(0x08);
        data.extend_from_slice(&3u32.to_be_bytes()); // count
        data.extend_from_slice(build_onmetadata_number("duration", 60.0).as_slice());
        data.extend_from_slice(build_onmetadata_number("width", 1920.0).as_slice());
        data.extend_from_slice(build_onmetadata_number("height", 1080.0).as_slice());
        // Object-end marker
        data.extend_from_slice(&[0x00, 0x00, 0x09]);

        let meta = parse_script_data(&data, AmfLimits::default()).unwrap();
        assert_eq!(meta.name, "onMetaData");
        assert_eq!(meta.width, Some(1920));
        assert_eq!(meta.height, Some(1080));
        assert_eq!(meta.duration_ms, Some(60_000));
    }
}
