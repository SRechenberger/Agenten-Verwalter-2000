//! Ausgaben von Shell-Befehlen als Text. Bash (unter Windows Git Bash) und PowerShell (mit UTF-8 als Ausgabe) liefern
//! UTF-8, Windows-Programme wie `ping` aber ihre OEM-Codepage — auch durch beide Shells hindurch
//! (`ausgef\x81hrt`). Gültiges UTF-8 bleibt deshalb, wie es ist; nur Bytes, die kein gültiges UTF-8
//! sind, werden als Codepage 850 (Westeuropa, die OEM-Codepage eines deutschen Windows) gelesen.
use std::str;

/// Codepage 850, Bytes 0x80–0xFF.
const CP850_HIGH: [char; 128] = [
    'Ç', 'ü', 'é', 'â', 'ä', 'à', 'å', 'ç', 'ê', 'ë', 'è', 'ï', 'î', 'ì', 'Ä', 'Å', //
    'É', 'æ', 'Æ', 'ô', 'ö', 'ò', 'û', 'ù', 'ÿ', 'Ö', 'Ü', 'ø', '£', 'Ø', '×', 'ƒ', //
    'á', 'í', 'ó', 'ú', 'ñ', 'Ñ', 'ª', 'º', '¿', '®', '¬', '½', '¼', '¡', '«', '»', //
    '░', '▒', '▓', '│', '┤', 'Á', 'Â', 'À', '©', '╣', '║', '╗', '╝', '¢', '¥', '┐', //
    '└', '┴', '┬', '├', '─', '┼', 'ã', 'Ã', '╚', '╔', '╩', '╦', '╠', '═', '╬', '¤', //
    'ð', 'Ð', 'Ê', 'Ë', 'È', 'ı', 'Í', 'Î', 'Ï', '┘', '┌', '█', '▄', '¦', 'Ì', '▀', //
    'Ó', 'ß', 'Ô', 'Ò', 'õ', 'Õ', 'µ', 'þ', 'Þ', 'Ú', 'Û', 'Ù', 'ý', 'Ý', '¯', '´', //
    '\u{ad}', '±', '‗', '¾', '¶', '§', '÷', '¸', '°', '¨', '·', '¹', '³', '²', '■', '\u{a0}',
];
const HIGH_START: u8 = 0x80;

/// Dekodiert einen Ausgabestrom in Stücken. Ein UTF-8-Zeichen, das an einer Stückgrenze
/// zerschnitten ist, wartet auf den Rest — je Strom ein eigener Decoder, sonst mischen sich die
/// Hälften von stdout und stderr.
#[derive(Default)]
pub struct OutputDecoder {
    /// Unvollständiges UTF-8-Zeichen am Ende des letzten Stücks.
    pending: Vec<u8>,
}

impl OutputDecoder {
    pub fn decode(&mut self, bytes: &[u8]) -> String {
        let mut input = std::mem::take(&mut self.pending);
        input.extend_from_slice(bytes);
        let mut text = String::with_capacity(input.len());
        let mut rest: &[u8] = &input;
        loop {
            let error = match str::from_utf8(rest) {
                Ok(valid) => {
                    text.push_str(valid);
                    return text;
                }
                Err(error) => error,
            };
            let (valid, invalid) = rest.split_at(error.valid_up_to());
            text.push_str(&String::from_utf8_lossy(valid));
            let Some(invalid_length) = error.error_len() else {
                // Das Stück endet mitten in einem Zeichen — der Rest kommt mit dem nächsten.
                self.pending = invalid.to_vec();
                return text;
            };
            text.extend(invalid[..invalid_length].iter().copied().map(oem_char));
            rest = &invalid[invalid_length..];
        }
    }

    /// Nach dem letzten Stück: ein angefangenes Zeichen kommt nicht mehr — es war keins.
    pub fn finish(&mut self) -> String {
        std::mem::take(&mut self.pending)
            .into_iter()
            .map(oem_char)
            .collect()
    }
}

fn oem_char(byte: u8) -> char {
    match byte.checked_sub(HIGH_START) {
        Some(index) => CP850_HIGH[usize::from(index)],
        None => char::from(byte),
    }
}
