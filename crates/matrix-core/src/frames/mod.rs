use crate::pixel::{Frame, Rgb, HEIGHT, PIXELS, WIDTH};
use std::io::{Read, Seek, SeekFrom, Write};

pub const LFX_MAGIC: &[u8; 4] = b"LFX1";
pub const LFX_HEADER_SIZE: usize = 11;
pub const BYTES_PER_FRAME: usize = PIXELS * 3; // 3072 bytes for 32x32 RGB888

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LfxHeader {
    pub width: u16,
    pub height: u16,
    pub frame_count: u16,
    pub format: u8, // 0 = RGB888
}

impl LfxHeader {
    pub fn new(frame_count: u16) -> Self {
        Self {
            width: WIDTH as u16,
            height: HEIGHT as u16,
            frame_count,
            format: 0,
        }
    }

    pub fn to_bytes(&self) -> [u8; LFX_HEADER_SIZE] {
        let mut buf = [0u8; LFX_HEADER_SIZE];
        buf[0..4].copy_from_slice(LFX_MAGIC);
        buf[4..6].copy_from_slice(&self.width.to_le_bytes());
        buf[6..8].copy_from_slice(&self.height.to_le_bytes());
        buf[8..10].copy_from_slice(&self.frame_count.to_le_bytes());
        buf[10] = self.format;
        buf
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < LFX_HEADER_SIZE {
            return Err("buffer too short for LFX header".to_string());
        }
        if &bytes[0..4] != LFX_MAGIC {
            return Err("invalid LFX magic: expected 'LFX1'".to_string());
        }
        let width = u16::from_le_bytes([bytes[4], bytes[5]]);
        let height = u16::from_le_bytes([bytes[6], bytes[7]]);
        let frame_count = u16::from_le_bytes([bytes[8], bytes[9]]);
        let format = bytes[10];

        if width as usize != WIDTH || height as usize != HEIGHT {
            return Err(format!(
                "unsupported frame dimensions {}x{}, expected {}x{}",
                width, height, WIDTH, HEIGHT
            ));
        }
        if format != 0 {
            return Err(format!(
                "unsupported LFX pixel format {}, expected 0 (RGB888)",
                format
            ));
        }
        if frame_count == 0 {
            return Err("LFX asset has 0 frames".to_string());
        }

        Ok(Self {
            width,
            height,
            frame_count,
            format,
        })
    }
}

/// Abstract streaming interface for reading animation frames.
/// Designed for low-memory microcontrollers (e.g. ESP32-WROOM):
/// reads individual frames without loading the entire animation into RAM.
pub trait FrameSource: Send {
    fn frame_count(&self) -> usize;
    fn read_frame_into(&mut self, index: usize, frame: &mut Frame) -> Result<(), String>;
}

/// A streaming LFX frame reader backed by any `Read + Seek` stream (file, memory cursor, etc.).
pub struct LfxStream<R: Read + Seek + Send> {
    reader: R,
    header: LfxHeader,
}

impl<R: Read + Seek + Send> LfxStream<R> {
    pub fn new(mut reader: R) -> Result<Self, String> {
        let mut hdr_buf = [0u8; LFX_HEADER_SIZE];
        reader
            .read_exact(&mut hdr_buf)
            .map_err(|e| format!("failed to read LFX header: {}", e))?;
        let header = LfxHeader::from_bytes(&hdr_buf)?;
        let expected_size =
            (LFX_HEADER_SIZE as u64) + (header.frame_count as u64) * (BYTES_PER_FRAME as u64);
        let stream_len = reader
            .seek(SeekFrom::End(0))
            .map_err(|e| format!("failed to determine stream length: {}", e))?;
        if stream_len < expected_size {
            return Err(format!(
                "LFX asset is truncated: size {} bytes, expected at least {} bytes for {} frames",
                stream_len, expected_size, header.frame_count
            ));
        }
        reader
            .seek(SeekFrom::Start(LFX_HEADER_SIZE as u64))
            .map_err(|e| format!("failed to seek to first frame: {}", e))?;
        Ok(Self { reader, header })
    }

    pub fn header(&self) -> &LfxHeader {
        &self.header
    }
}

impl<R: Read + Seek + Send> FrameSource for LfxStream<R> {
    fn frame_count(&self) -> usize {
        self.header.frame_count as usize
    }

    fn read_frame_into(&mut self, index: usize, frame: &mut Frame) -> Result<(), String> {
        if self.header.frame_count == 0 {
            frame.clear(Rgb::BLACK);
            return Ok(());
        }
        let target_index = index % (self.header.frame_count as usize);
        let offset = LFX_HEADER_SIZE as u64 + (target_index as u64) * (BYTES_PER_FRAME as u64);

        self.reader
            .seek(SeekFrom::Start(offset))
            .map_err(|e| format!("failed to seek to frame {}: {}", target_index, e))?;

        let mut buf = [0u8; BYTES_PER_FRAME];
        self.reader
            .read_exact(&mut buf)
            .map_err(|e| format!("failed to read frame {}: {}", target_index, e))?;

        for i in 0..PIXELS {
            let r = buf[i * 3];
            let g = buf[i * 3 + 1];
            let b = buf[i * 3 + 2];
            frame.pixels[i] = Rgb::new(r, g, b);
        }
        Ok(())
    }
}

/// In-memory frame source backed by raw LFX or RGB888 bytes (e.g. PROGMEM or pre-loaded slice).
pub struct SliceFrameSource<'a> {
    data: &'a [u8],
    frame_count: usize,
    data_offset: usize,
}

impl<'a> SliceFrameSource<'a> {
    pub fn from_lfx_bytes(data: &'a [u8]) -> Result<Self, String> {
        let header = LfxHeader::from_bytes(data)?;
        let count = header.frame_count as usize;
        let required = LFX_HEADER_SIZE + count * BYTES_PER_FRAME;
        if data.len() < required {
            return Err(format!(
                "slice too short: {} bytes, required {} for {} frames",
                data.len(),
                required,
                count
            ));
        }
        Ok(Self {
            data,
            frame_count: count,
            data_offset: LFX_HEADER_SIZE,
        })
    }

    pub fn from_raw_rgb_bytes(data: &'a [u8]) -> Result<Self, String> {
        if !data.len().is_multiple_of(BYTES_PER_FRAME) {
            return Err("slice length must be a multiple of BYTES_PER_FRAME (3072)".to_string());
        }
        let count = data.len() / BYTES_PER_FRAME;
        Ok(Self {
            data,
            frame_count: count,
            data_offset: 0,
        })
    }
}

impl<'a> FrameSource for SliceFrameSource<'a> {
    fn frame_count(&self) -> usize {
        self.frame_count
    }

    fn read_frame_into(&mut self, index: usize, frame: &mut Frame) -> Result<(), String> {
        if self.frame_count == 0 {
            frame.clear(Rgb::BLACK);
            return Ok(());
        }
        let idx = index % self.frame_count;
        let start = self.data_offset + idx * BYTES_PER_FRAME;
        let end = start + BYTES_PER_FRAME;
        let slice = &self.data[start..end];

        for i in 0..PIXELS {
            let r = slice[i * 3];
            let g = slice[i * 3 + 1];
            let b = slice[i * 3 + 2];
            frame.pixels[i] = Rgb::new(r, g, b);
        }
        Ok(())
    }
}

/// Helper to write frames into an LFX stream.
pub struct LfxWriter<W: Write> {
    writer: W,
    frames_written: usize,
}

impl<W: Write> LfxWriter<W> {
    pub fn new(mut writer: W, total_frames: u16) -> Result<Self, String> {
        let header = LfxHeader::new(total_frames);
        writer
            .write_all(&header.to_bytes())
            .map_err(|e| format!("failed to write LFX header: {}", e))?;
        Ok(Self {
            writer,
            frames_written: 0,
        })
    }

    pub fn write_frame(&mut self, frame: &Frame) -> Result<(), String> {
        let mut buf = [0u8; BYTES_PER_FRAME];
        for i in 0..PIXELS {
            buf[i * 3] = frame.pixels[i].r;
            buf[i * 3 + 1] = frame.pixels[i].g;
            buf[i * 3 + 2] = frame.pixels[i].b;
        }
        self.writer
            .write_all(&buf)
            .map_err(|e| format!("failed to write frame: {}", e))?;
        self.frames_written += 1;
        Ok(())
    }
}

/// Parses additive JSON frame format: array of frames, where each frame is either:
/// 1. Array of 1024 [r, g, b] arrays
/// 2. Array of 1024 hex numbers or strings
/// 3. Object with "pixels" or "frames"
pub fn parse_json_frame(val: &serde_json::Value, frame: &mut Frame) -> Result<(), String> {
    let pixels_arr = if let Some(arr) = val.as_array() {
        arr
    } else if let Some(arr) = val.get("pixels").and_then(|v| v.as_array()) {
        arr
    } else {
        return Err("expected array of pixels or object with 'pixels' array".to_string());
    };

    if pixels_arr.len() < PIXELS {
        return Err(format!(
            "expected at least {} pixels, got {}",
            PIXELS,
            pixels_arr.len()
        ));
    }

    for (i, p) in pixels_arr.iter().take(PIXELS).enumerate() {
        if let Some(rgb_arr) = p.as_array() {
            if rgb_arr.len() >= 3 {
                let r = rgb_arr[0].as_u64().unwrap_or(0).min(255) as u8;
                let g = rgb_arr[1].as_u64().unwrap_or(0).min(255) as u8;
                let b = rgb_arr[2].as_u64().unwrap_or(0).min(255) as u8;
                frame.pixels[i] = Rgb::new(r, g, b);
            }
        } else if let Some(hex_str) = p.as_str() {
            let hex_clean = hex_str.trim_start_matches('#');
            if let Ok(num) = u32::from_str_radix(hex_clean, 16) {
                let r = ((num >> 16) & 0xFF) as u8;
                let g = ((num >> 8) & 0xFF) as u8;
                let b = (num & 0xFF) as u8;
                frame.pixels[i] = Rgb::new(r, g, b);
            }
        } else if let Some(num) = p.as_u64() {
            let r = ((num >> 16) & 0xFF) as u8;
            let g = ((num >> 8) & 0xFF) as u8;
            let b = (num & 0xFF) as u8;
            frame.pixels[i] = Rgb::new(r, g, b);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_lfx_header_roundtrip() {
        let header = LfxHeader::new(42);
        let bytes = header.to_bytes();
        assert_eq!(bytes.len(), 11);
        let parsed = LfxHeader::from_bytes(&bytes).unwrap();
        assert_eq!(parsed, header);
        assert_eq!(parsed.frame_count, 42);
    }

    #[test]
    fn test_lfx_stream_read_write() {
        let mut buffer = Vec::new();
        {
            let mut writer = LfxWriter::new(&mut buffer, 2).unwrap();
            let mut f1 = Frame::default();
            f1.set(0, 0, Rgb::new(10, 20, 30));
            writer.write_frame(&f1).unwrap();

            let mut f2 = Frame::default();
            f2.set(31, 31, Rgb::new(40, 50, 60));
            writer.write_frame(&f2).unwrap();
        }

        assert_eq!(buffer.len(), LFX_HEADER_SIZE + 2 * BYTES_PER_FRAME);

        let mut stream = LfxStream::new(Cursor::new(buffer)).unwrap();
        assert_eq!(stream.frame_count(), 2);

        let mut read_f1 = Frame::default();
        stream.read_frame_into(0, &mut read_f1).unwrap();
        assert_eq!(read_f1.get(0, 0), Rgb::new(10, 20, 30));

        let mut read_f2 = Frame::default();
        stream.read_frame_into(1, &mut read_f2).unwrap();
        assert_eq!(read_f2.get(31, 31), Rgb::new(40, 50, 60));
    }

    #[test]
    fn test_lfx_rejects_zero_frames_and_truncation() {
        // Zero frames
        let zero_hdr = LfxHeader {
            width: 32,
            height: 32,
            frame_count: 0,
            format: 0,
        };
        assert!(LfxHeader::from_bytes(&zero_hdr.to_bytes()).is_err());
        assert!(LfxStream::new(Cursor::new(zero_hdr.to_bytes())).is_err());

        // Truncated asset (header says 2 frames, but only 1 frame provided)
        let mut buffer = Vec::new();
        {
            let mut writer = LfxWriter::new(&mut buffer, 2).unwrap();
            let f1 = Frame::default();
            writer.write_frame(&f1).unwrap();
        }
        // Only 1 frame was written, expected 2
        assert!(LfxStream::new(Cursor::new(buffer)).is_err());
    }
}
