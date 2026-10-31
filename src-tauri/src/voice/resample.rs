//! Umrechnung der Aufnahme auf das Format, das die Erkennung erwartet: 16 kHz mono.

pub const TARGET_RATE: u32 = 16_000;

/// Mittelwert der Kanäle je Frame, dann lineare Interpolation auf 16 kHz. Ohne Tiefpass davor: Anteile
/// über 8 kHz falten sich als leises Rauschen zurück — beim Anhören störend, für die Spracherkennung
/// bedeutungslos. Wird je Abschnitt aufgerufen, nicht je 50-ms-Stück, damit an den Stückgrenzen
/// keine Sprünge entstehen.
pub fn to_mono_16k(interleaved: &[f32], channels: u16, rate: u32) -> Vec<f32> {
    let mono = mix_to_mono(interleaved, channels);
    if rate == TARGET_RATE || mono.is_empty() {
        return mono;
    }
    let input_len = mono.len();
    let last_index = input_len - 1;
    let output_len = (input_len as u64 * u64::from(TARGET_RATE) / u64::from(rate)) as usize;
    let step = f64::from(rate) / f64::from(TARGET_RATE);
    (0..output_len)
        .map(|output_index: usize| {
            let position = output_index as f64 * step;
            let floor = (position.floor() as usize).min(last_index);
            let fraction = (position - floor as f64) as f32;
            let current = mono[floor];
            let next = mono[(floor + 1).min(last_index)];
            current + (next - current) * fraction
        })
        .collect()
}

fn mix_to_mono(interleaved: &[f32], channels: u16) -> Vec<f32> {
    if channels <= 1 {
        return interleaved.to_vec();
    }
    let channel_count = usize::from(channels);
    interleaved
        .chunks_exact(channel_count)
        .map(|frame: &[f32]| frame.iter().sum::<f32>() / channel_count as f32)
        .collect()
}
