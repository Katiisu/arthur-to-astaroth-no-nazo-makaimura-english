//! "Graveyard Shift" - an original chiptune loop, synthesized at startup.
//! No audio files are shipped: the tune is written out as notes below and rendered
//! to an in-memory WAV, which Windows plays with its built-in PlaySound API.

const RATE: u32 = 32_000;
const BPM: f32 = 132.0;

// MIDI note numbers. 0 = rest.
const R: u8 = 0;
const A4: u8 = 69;
const BB4: u8 = 70;
const C5: u8 = 72;
const CS5: u8 = 73;
const D5: u8 = 74;
const E5: u8 = 76;
const F5: u8 = 77;
const G5: u8 = 79;
const A5: u8 = 81;
const BB5: u8 = 82;
const C6: u8 = 84;
const CS6: u8 = 85;
const D6: u8 = 86;
const E6: u8 = 88;

/// Lead melody: (note, length in 16th notes). Each bar adds up to 16.
const MELODY: &[&[(u8, u8)]] = &[
    // A - D minor, Bb, C, A
    &[
        (D5, 4),
        (F5, 2),
        (A5, 2),
        (G5, 2),
        (F5, 2),
        (E5, 2),
        (F5, 2),
    ],
    &[(F5, 2), (D5, 2), (BB4, 4), (D5, 2), (F5, 2), (BB5, 4)],
    &[(G5, 4), (E5, 2), (C5, 2), (E5, 2), (G5, 2), (C6, 4)],
    &[(A5, 6), (G5, 2), (F5, 2), (E5, 2), (CS5, 4)],
    // A'
    &[
        (D5, 4),
        (F5, 2),
        (A5, 2),
        (G5, 2),
        (F5, 2),
        (E5, 2),
        (F5, 2),
    ],
    &[(F5, 2), (D5, 2), (BB4, 4), (D5, 2), (F5, 2), (BB5, 4)],
    &[(C6, 2), (BB5, 2), (G5, 2), (E5, 2), (G5, 4), (E5, 4)],
    &[(E5, 2), (F5, 2), (E5, 2), (D5, 2), (CS5, 4), (A4, 4)],
    // B - climbs higher
    &[(A5, 2), (A5, 2), (D6, 4), (C6, 2), (A5, 2), (F5, 4)],
    &[(BB5, 2), (BB5, 2), (D6, 4), (C6, 2), (BB5, 2), (F5, 4)],
    &[(G5, 2), (A5, 2), (BB5, 2), (C6, 2), (E6, 4), (C6, 4)],
    &[(CS6, 4), (E6, 4), (A5, 8)],
    // A'' - winds down, loops back to the top
    &[
        (D5, 4),
        (F5, 2),
        (A5, 2),
        (G5, 2),
        (F5, 2),
        (E5, 2),
        (F5, 2),
    ],
    &[(F5, 2), (D5, 2), (BB4, 4), (D5, 2), (F5, 2), (BB5, 4)],
    &[(E5, 2), (G5, 2), (C6, 2), (E6, 2), (D6, 4), (C6, 4)],
    &[(CS6, 2), (A5, 2), (E5, 2), (CS5, 2), (A4, 4), (R, 4)],
];

/// Chord per bar as (bass root, arpeggio notes).
const CHORDS: [(u8, [u8; 4]); 4] = [
    (38, [62, 65, 69, 74]), // D minor
    (34, [58, 62, 65, 70]), // Bb major
    (36, [60, 64, 67, 72]), // C major
    (33, [57, 61, 64, 69]), // A major
];

fn freq(n: u8) -> f32 {
    440.0 * 2f32.powf((n as f32 - 69.0) / 12.0)
}

/// Renders the whole loop as 16-bit mono PCM samples.
pub fn render() -> Vec<i16> {
    let step = 60.0 / BPM / 4.0; // one 16th note, in seconds
    let steps = MELODY.len() * 16;
    let total = (steps as f32 * step * RATE as f32) as usize;
    let mut mix = vec![0f32; total];

    let add_note =
        |start_step: usize, len_steps: f32, note: u8, vol: f32, wave: u8, mix: &mut Vec<f32>| {
            if note == R {
                return;
            }
            let f = freq(note);
            let s0 = (start_step as f32 * step * RATE as f32) as usize;
            let n = (len_steps * step * RATE as f32) as usize;
            let release = (0.02 * RATE as f32) as usize;
            let mut phase = 0f32;
            for i in 0..n {
                let t = i as f32 / RATE as f32;
                // gentle vibrato on long lead notes
                let vib = if wave == 0 && t > 0.18 {
                    1.0 + 0.004 * (t * 34.0).sin()
                } else {
                    1.0
                };
                phase = (phase + f * vib / RATE as f32).fract();
                let raw = match wave {
                    0 => {
                        if phase < 0.25 {
                            1.0
                        } else {
                            -1.0
                        }
                    } // lead: 25% pulse
                    1 => {
                        if phase < 0.125 {
                            1.0
                        } else {
                            -1.0
                        }
                    } // arpeggio: thin 12.5% pulse
                    _ => 4.0 * (phase - 0.5).abs() - 1.0, // bass: triangle
                };
                let attack = (i as f32 / (0.004 * RATE as f32)).min(1.0);
                let decay = 0.65 + 0.35 * (-t * 6.0).exp();
                let tail = if i + release > n {
                    (n - i) as f32 / release as f32
                } else {
                    1.0
                };
                if let Some(m) = mix.get_mut(s0 + i) {
                    *m += raw * vol * attack * decay * tail;
                }
            }
        };

    // Lead
    let mut pos = 0usize;
    for bar in MELODY {
        for &(note, len) in bar.iter() {
            add_note(pos, len as f32 * 0.92, note, 0.20, 0, &mut mix);
            pos += len as usize;
        }
    }
    // Bass and arpeggio
    for bar in 0..MELODY.len() {
        let (root, arp) = CHORDS[bar % 4];
        for k in 0..8 {
            let note = if k % 2 == 0 { root } else { root + 12 };
            add_note(bar * 16 + k * 2, 1.8, note, 0.26, 2, &mut mix);
        }
        for k in 0..16 {
            add_note(bar * 16 + k, 0.8, arp[k % 4], 0.045, 1, &mut mix);
        }
    }
    // Light percussion from a cheap noise generator
    let mut seed: u32 = 0x1234_5678;
    let mut noise = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed as f32 / u32::MAX as f32) * 2.0 - 1.0
    };
    for s in 0..steps {
        let in_bar = s % 16;
        let (len, vol, decay) = match in_bar {
            4 | 12 => (0.10, 0.10, 30.0),                  // snare
            0 | 8 => (0.06, 0.06, 60.0),                   // kick-ish thump
            _ if in_bar % 2 == 1 => (0.025, 0.025, 120.0), // hi-hat
            _ => continue,
        };
        let s0 = (s as f32 * step * RATE as f32) as usize;
        let n = (len * RATE as f32) as usize;
        for i in 0..n {
            let t = i as f32 / RATE as f32;
            if let Some(m) = mix.get_mut(s0 + i) {
                *m += noise() * vol * (-t * decay).exp();
            }
        }
    }

    mix.iter()
        .map(|&x| {
            let y = (x * 1.1).tanh(); // soft clip
            (y * 0.85 * i16::MAX as f32) as i16
        })
        .collect()
}

/// Wraps PCM samples in a minimal WAV container.
pub fn wav_bytes(samples: &[i16]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut w = Vec::with_capacity(44 + data_len as usize);
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&(36 + data_len).to_le_bytes());
    w.extend_from_slice(b"WAVEfmt ");
    w.extend_from_slice(&16u32.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes()); // PCM
    w.extend_from_slice(&1u16.to_le_bytes()); // mono
    w.extend_from_slice(&RATE.to_le_bytes());
    w.extend_from_slice(&(RATE * 2).to_le_bytes());
    w.extend_from_slice(&2u16.to_le_bytes());
    w.extend_from_slice(&16u16.to_le_bytes());
    w.extend_from_slice(b"data");
    w.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        w.extend_from_slice(&s.to_le_bytes());
    }
    w
}

/// Background music player. Plays only on Windows; elsewhere it stays silent.
pub struct Music {
    #[allow(dead_code)]
    wav: &'static [u8],
    pub playing: bool,
}

impl Music {
    pub fn new() -> Self {
        let wav: &'static [u8] = Box::leak(wav_bytes(&render()).into_boxed_slice());
        Music {
            wav,
            playing: false,
        }
    }

    pub fn set(&mut self, on: bool) {
        if on == self.playing {
            return;
        }
        self.playing = on;
        platform::play(if on { Some(self.wav) } else { None });
    }
}

#[cfg(windows)]
mod platform {
    #[link(name = "winmm")]
    extern "system" {
        fn PlaySoundA(psz: *const u8, hmod: *mut core::ffi::c_void, flags: u32) -> i32;
    }
    const SND_ASYNC: u32 = 0x0001;
    const SND_NODEFAULT: u32 = 0x0002;
    const SND_MEMORY: u32 = 0x0004;
    const SND_LOOP: u32 = 0x0008;

    pub fn play(wav: Option<&'static [u8]>) {
        unsafe {
            match wav {
                Some(w) => {
                    PlaySoundA(
                        w.as_ptr(),
                        core::ptr::null_mut(),
                        SND_MEMORY | SND_ASYNC | SND_LOOP | SND_NODEFAULT,
                    );
                }
                None => {
                    PlaySoundA(core::ptr::null(), core::ptr::null_mut(), 0);
                }
            }
        }
    }
}

#[cfg(not(windows))]
mod platform {
    pub fn play(_wav: Option<&'static [u8]>) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bar_is_one_full_bar() {
        for (i, bar) in MELODY.iter().enumerate() {
            let len: u32 = bar.iter().map(|&(_, l)| l as u32).sum();
            assert_eq!(len, 16, "bar {} has {} sixteenths", i + 1, len);
        }
    }

    #[test]
    fn renders_a_clean_loop() {
        let pcm = render();
        let secs = pcm.len() as f32 / RATE as f32;
        assert!(
            (secs - 16.0 * 4.0 * 60.0 / BPM).abs() < 0.05,
            "loop is {secs}s"
        );
        let peak = pcm.iter().map(|s| s.unsigned_abs()).max().unwrap();
        assert!(peak < i16::MAX as u16, "clipping");
        assert!(peak > 8000, "too quiet");
    }
}
