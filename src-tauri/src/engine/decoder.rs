//! Turns raw process output bytes into lines.
//! Handles UTF-8 and the UTF-16LE some Windows tools (sfc, DISM in some modes) emit when redirected,
//! and distinguishes `\r` in-place progress updates from real line breaks.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Encoding {
    Unknown,
    Utf8,
    Utf16,
}

#[derive(Debug, PartialEq, Eq)]
pub struct DecodedLine {
    pub text: String,
    pub transient: bool,
}

pub struct LineDecoder {
    encoding: Encoding,
    bytes: Vec<u8>,
    text: String,
}

impl LineDecoder {
    pub fn new() -> Self {
        Self { encoding: Encoding::Unknown, bytes: Vec::new(), text: String::new() }
    }

    fn detect(chunk: &[u8]) -> Encoding {
        if chunk.starts_with(&[0xFF, 0xFE]) {
            return Encoding::Utf16;
        }
        // ASCII text in UTF-16LE has a zero in (nearly) every odd byte.
        let odd = chunk.iter().skip(1).step_by(2).take(256);
        let (zeros, total) = odd.fold((0, 0), |(z, t), b| (z + (*b == 0) as usize, t + 1));
        if total >= 2 && zeros * 10 >= total * 7 {
            Encoding::Utf16
        } else {
            Encoding::Utf8
        }
    }

    /// Feeds bytes and returns all complete lines.
    pub fn push(&mut self, chunk: &[u8]) -> Vec<DecodedLine> {
        if chunk.is_empty() {
            return Vec::new();
        }
        if self.encoding == Encoding::Unknown {
            self.encoding = Self::detect(chunk);
        }
        self.bytes.extend_from_slice(chunk);

        match self.encoding {
            Encoding::Utf16 => {
                let mut start = 0;
                if self.bytes.starts_with(&[0xFF, 0xFE]) {
                    start = 2;
                }
                let usable = (self.bytes.len() - start) / 2 * 2;
                let units: Vec<u16> = self.bytes[start..start + usable]
                    .as_chunks::<2>().0.iter()
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .collect();
                // Keep a trailing high surrogate for the next chunk.
                let keep_surrogate = units.last().is_some_and(|u| (0xD800..0xDC00).contains(u));
                let n = if keep_surrogate { units.len() - 1 } else { units.len() };
                self.text.push_str(&String::from_utf16_lossy(&units[..n]));
                self.bytes.drain(..start + n * 2);
            }
            _ => {
                let mut start = 0;
                if self.bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
                    start = 3;
                }
                match std::str::from_utf8(&self.bytes[start..]) {
                    Ok(s) => {
                        self.text.push_str(s);
                        self.bytes.clear();
                    }
                    Err(e) => {
                        let valid = start + e.valid_up_to();
                        if e.error_len().is_none() {
                            // Incomplete multi-byte sequence at the end: wait for more bytes.
                            self.text.push_str(std::str::from_utf8(&self.bytes[start..valid]).unwrap_or_default());
                            self.bytes.drain(..valid);
                        } else {
                            // Not UTF-8 (e.g. an OEM code page): decode lossily rather than stall.
                            self.text.push_str(&String::from_utf8_lossy(&self.bytes[start..]));
                            self.bytes.clear();
                        }
                    }
                }
            }
        }
        self.split_lines(false)
    }

    /// Returns whatever is left once the stream has ended.
    pub fn finish(&mut self) -> Vec<DecodedLine> {
        if !self.bytes.is_empty() {
            let rest = std::mem::take(&mut self.bytes);
            self.text.push_str(&String::from_utf8_lossy(&rest));
        }
        self.split_lines(true)
    }

    fn split_lines(&mut self, eof: bool) -> Vec<DecodedLine> {
        let mut out = Vec::new();
        let text = std::mem::take(&mut self.text);

        let mut line = String::new();
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '\n' => out.push(DecodedLine { text: std::mem::take(&mut line), transient: false }),
                '\r' => match chars.peek() {
                    Some('\n') => {
                        chars.next();
                        out.push(DecodedLine { text: std::mem::take(&mut line), transient: false });
                    }
                    Some(_) => out.push(DecodedLine { text: std::mem::take(&mut line), transient: true }),
                    None if eof => out.push(DecodedLine { text: std::mem::take(&mut line), transient: false }),
                    // Can't tell `\r\n` from a lone `\r` yet: keep it for the next chunk.
                    None => line.push('\r'),
                },
                _ => line.push(c),
            }
        }
        if eof {
            if !line.is_empty() {
                out.push(DecodedLine { text: line, transient: false });
            }
        } else {
            self.text = line;
        }

        // Trim trailing whitespace and drop empty in-place updates (common between progress redraws).
        out.into_iter()
            .map(|mut l| {
                l.text.truncate(l.text.trim_end().len());
                l
            })
            .filter(|l| !(l.transient && l.text.is_empty()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(lines: &[DecodedLine]) -> Vec<(&str, bool)> {
        lines.iter().map(|l| (l.text.as_str(), l.transient)).collect()
    }

    #[test]
    fn utf8_lines_across_chunks() {
        let mut d = LineDecoder::new();
        let mut all = d.push(b"hello\r\nwor");
        all.extend(d.push(b"ld\nlast"));
        all.extend(d.finish());
        assert_eq!(texts(&all), vec![("hello", false), ("world", false), ("last", false)]);
    }

    #[test]
    fn split_crlf_at_chunk_boundary_is_one_line() {
        let mut d = LineDecoder::new();
        let mut all = d.push(b"a\r");
        all.extend(d.push(b"\nb\n"));
        assert_eq!(texts(&all), vec![("a", false), ("b", false)]);
    }

    #[test]
    fn carriage_return_progress_is_transient() {
        let mut d = LineDecoder::new();
        let all = d.push(b"10%\r20%\rdone\r\n");
        assert_eq!(texts(&all), vec![("10%", true), ("20%", true), ("done", false)]);
    }

    #[test]
    fn utf16_output_is_decoded() {
        let mut d = LineDecoder::new();
        let bytes: Vec<u8> = "Verification 5% complete.\r\nok\r\n".encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        let (a, b) = bytes.split_at(7); // odd split inside a code unit
        let mut all = d.push(a);
        all.extend(d.push(b));
        assert_eq!(texts(&all), vec![("Verification 5% complete.", false), ("ok", false)]);
    }

    #[test]
    fn utf8_multibyte_split() {
        let mut d = LineDecoder::new();
        let s = "°C\n".as_bytes();
        let mut all = d.push(&s[..1]);
        all.extend(d.push(&s[1..]));
        assert_eq!(texts(&all), vec![("°C", false)]);
    }
}
