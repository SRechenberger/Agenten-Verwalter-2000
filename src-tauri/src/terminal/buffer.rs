//! Bytes aus dem Pseudo-Terminal als Text und der Verlauf, den ein neu angehängter Kanal nachgespielt bekommt.

/// Setzt UTF-8 aus Lesestücken zusammen: ein Zeichen, das an der Stückgrenze zerrissen wird, wartet
/// auf sein Ende; ungültige Bytes werden zu U+FFFD.
#[derive(Default)]
pub struct Utf8Carry {
    pending: Vec<u8>,
}

impl Utf8Carry {
    pub fn decode(&mut self, bytes: &[u8]) -> String {
        let mut input: Vec<u8> = std::mem::take(&mut self.pending);
        input.extend_from_slice(bytes);
        let mut text = String::with_capacity(input.len());
        let mut rest: &[u8] = &input;
        loop {
            let error = match std::str::from_utf8(rest) {
                Ok(valid) => {
                    text.push_str(valid);
                    break;
                }
                Err(error) => error,
            };
            let (valid, after) = rest.split_at(error.valid_up_to());
            text.push_str(&String::from_utf8_lossy(valid));
            match error.error_len() {
                Some(skipped) => {
                    text.push('\u{FFFD}');
                    rest = &after[skipped..];
                }
                None => {
                    // Unvollständiges Zeichen am Ende (höchstens 3 Bytes): das nächste Stück vollendet es.
                    self.pending = after.to_vec();
                    break;
                }
            }
        }
        text
    }
}

const CAPACITY: usize = 1024 * 1024;

/// Die letzten `CAPACITY` Bytes der Ausgabe.
#[derive(Default)]
pub struct ScrollBuffer {
    text: String,
}

impl ScrollBuffer {
    pub fn push(&mut self, chunk: &str) {
        self.text.push_str(chunk);
        if self.text.len() <= CAPACITY {
            return;
        }
        let mut cut: usize = self.text.len() - CAPACITY;
        while !self.text.is_char_boundary(cut) {
            cut += 1;
        }
        self.text.drain(..cut);
    }

    pub fn snapshot(&self) -> String {
        self.text.clone()
    }
}
