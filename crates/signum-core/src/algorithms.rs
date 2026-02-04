//! Algorithmic composition tools (Euclidean rhythms, chord generation)

use serde::{Deserialize, Serialize};

// ============================================================================
// Euclidean Rhythm Generator
// ============================================================================

/// Generate a Euclidean rhythm pattern
///
/// # Arguments
/// * `steps` - Total number of steps in the pattern (e.g., 16)
/// * `hits` - Number of hits to distribute (e.g., 5)
/// * `rotation` - Rotate pattern by this many steps
///
/// # Returns
/// Vec of bools where true = hit, false = rest
///
/// # Example
/// ```
/// use signum_core::euclidean_rhythm;
/// let pattern = euclidean_rhythm(8, 3, 0);
/// // Returns [true, false, false, true, false, false, true, false]
/// ```
pub fn euclidean_rhythm(steps: u8, hits: u8, rotation: u8) -> Vec<bool> {
    if steps == 0 {
        return vec![];
    }

    let hits = hits.min(steps);

    if hits == 0 {
        return vec![false; steps as usize];
    }

    if hits == steps {
        return vec![true; steps as usize];
    }

    // Bjorklund's algorithm
    let mut pattern = Vec::with_capacity(steps as usize);
    let mut counts = vec![vec![true]; hits as usize];
    let mut remainders = vec![vec![false]; (steps - hits) as usize];

    loop {
        let mut new_counts = Vec::new();

        let pairs = counts.len().min(remainders.len());
        for i in 0..pairs {
            let mut combined = counts[i].clone();
            combined.extend(remainders[i].clone());
            new_counts.push(combined);
        }

        // Leftover counts
        if counts.len() > pairs {
            remainders = counts[pairs..].to_vec();
        } else {
            remainders = remainders[pairs..].to_vec();
        }

        counts = new_counts;

        if remainders.len() <= 1 {
            break;
        }
    }

    // Combine final pattern
    for seq in &counts {
        pattern.extend(seq);
    }
    for seq in &remainders {
        pattern.extend(seq);
    }

    // Apply rotation
    if rotation > 0 {
        let rot = (rotation as usize) % pattern.len();
        pattern.rotate_left(rot);
    }

    pattern
}

// ============================================================================
// Scale and Chord Types
// ============================================================================

/// Scale/mode types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScaleMode {
    Major,
    Minor,
    Dorian,
    Phrygian,
    Lydian,
    Mixolydian,
    Locrian,
    HarmonicMinor,
    MelodicMinor,
    Pentatonic,
    Blues,
    Chromatic,
}

impl ScaleMode {
    /// Get scale intervals (semitones from root)
    pub fn intervals(&self) -> &'static [u8] {
        match self {
            Self::Major => &[0, 2, 4, 5, 7, 9, 11],
            Self::Minor => &[0, 2, 3, 5, 7, 8, 10],
            Self::Dorian => &[0, 2, 3, 5, 7, 9, 10],
            Self::Phrygian => &[0, 1, 3, 5, 7, 8, 10],
            Self::Lydian => &[0, 2, 4, 6, 7, 9, 11],
            Self::Mixolydian => &[0, 2, 4, 5, 7, 9, 10],
            Self::Locrian => &[0, 1, 3, 5, 6, 8, 10],
            Self::HarmonicMinor => &[0, 2, 3, 5, 7, 8, 11],
            Self::MelodicMinor => &[0, 2, 3, 5, 7, 9, 11],
            Self::Pentatonic => &[0, 2, 4, 7, 9],
            Self::Blues => &[0, 3, 5, 6, 7, 10],
            Self::Chromatic => &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Major => "Major",
            Self::Minor => "Minor",
            Self::Dorian => "Dorian",
            Self::Phrygian => "Phrygian",
            Self::Lydian => "Lydian",
            Self::Mixolydian => "Mixolydian",
            Self::Locrian => "Locrian",
            Self::HarmonicMinor => "Harmonic Minor",
            Self::MelodicMinor => "Melodic Minor",
            Self::Pentatonic => "Pentatonic",
            Self::Blues => "Blues",
            Self::Chromatic => "Chromatic",
        }
    }
}

/// Chord voicing types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Voicing {
    Close,        // Notes stacked in order
    Open,         // Spread over octaves
    Drop2,        // 2nd voice from top dropped an octave
    Drop3,        // 3rd voice from top dropped an octave
    RootBass,     // Root in bass, rest voiced freely
}

/// Chord quality
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChordQuality {
    Major,
    Minor,
    Diminished,
    Augmented,
    Major7,
    Minor7,
    Dominant7,
    Diminished7,
    HalfDiminished7,
    Sus2,
    Sus4,
    Add9,
}

impl ChordQuality {
    /// Get chord intervals from root
    pub fn intervals(&self) -> &'static [u8] {
        match self {
            Self::Major => &[0, 4, 7],
            Self::Minor => &[0, 3, 7],
            Self::Diminished => &[0, 3, 6],
            Self::Augmented => &[0, 4, 8],
            Self::Major7 => &[0, 4, 7, 11],
            Self::Minor7 => &[0, 3, 7, 10],
            Self::Dominant7 => &[0, 4, 7, 10],
            Self::Diminished7 => &[0, 3, 6, 9],
            Self::HalfDiminished7 => &[0, 3, 6, 10],
            Self::Sus2 => &[0, 2, 7],
            Self::Sus4 => &[0, 5, 7],
            Self::Add9 => &[0, 4, 7, 14],
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Major => "Major",
            Self::Minor => "Minor",
            Self::Diminished => "Dim",
            Self::Augmented => "Aug",
            Self::Major7 => "Maj7",
            Self::Minor7 => "Min7",
            Self::Dominant7 => "Dom7",
            Self::Diminished7 => "Dim7",
            Self::HalfDiminished7 => "m7b5",
            Self::Sus2 => "Sus2",
            Self::Sus4 => "Sus4",
            Self::Add9 => "Add9",
        }
    }
}

// ============================================================================
// Chord Generator
// ============================================================================

/// Chord generator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChordGenerator {
    pub root: u8,
    pub quality: ChordQuality,
    pub voicing: Voicing,
    pub octave: i8,
    pub inversion: u8,
}

impl Default for ChordGenerator {
    fn default() -> Self {
        Self {
            root: 60,  // Middle C
            quality: ChordQuality::Major,
            voicing: Voicing::Close,
            octave: 0,
            inversion: 0,
        }
    }
}

impl ChordGenerator {
    pub fn new(root: u8, quality: ChordQuality) -> Self {
        Self {
            root,
            quality,
            ..Default::default()
        }
    }

    /// Generate chord MIDI notes
    pub fn generate(&self) -> Vec<u8> {
        let intervals = self.quality.intervals();
        let mut notes: Vec<u8> = intervals
            .iter()
            .map(|&interval| {
                let note = self.root as i16 + interval as i16 + (self.octave as i16 * 12);
                note.clamp(0, 127) as u8
            })
            .collect();

        // Apply inversion
        let inv = (self.inversion as usize) % notes.len();
        for _ in 0..inv {
            if let Some(&first) = notes.first() {
                notes.remove(0);
                notes.push(first + 12);
            }
        }

        // Apply voicing
        match self.voicing {
            Voicing::Close => {}  // Already in close position
            Voicing::Open => {
                // Every other note up an octave
                for (i, note) in notes.iter_mut().enumerate() {
                    if i % 2 == 1 {
                        *note = (*note + 12).min(127);
                    }
                }
            }
            Voicing::Drop2 => {
                // Drop 2nd from top down an octave
                if notes.len() >= 2 {
                    let idx = notes.len() - 2;
                    notes[idx] = notes[idx].saturating_sub(12);
                    notes.sort();
                }
            }
            Voicing::Drop3 => {
                // Drop 3rd from top down an octave
                if notes.len() >= 3 {
                    let idx = notes.len() - 3;
                    notes[idx] = notes[idx].saturating_sub(12);
                    notes.sort();
                }
            }
            Voicing::RootBass => {
                // Ensure root is lowest
                notes.sort();
                if let Some(root_idx) = notes.iter().position(|&n| n % 12 == self.root % 12) {
                    if root_idx > 0 {
                        let root_note = notes.remove(root_idx);
                        notes.insert(0, root_note.saturating_sub(12));
                    }
                }
            }
        }

        notes
    }

    /// Get chord from scale degree
    pub fn from_scale_degree(scale_root: u8, scale: ScaleMode, degree: u8, quality: Option<ChordQuality>) -> Self {
        let intervals = scale.intervals();
        let degree_idx = ((degree - 1) as usize) % intervals.len();
        let root = scale_root + intervals[degree_idx];

        // Auto-detect quality from scale if not specified
        let quality = quality.unwrap_or_else(|| {
            match scale {
                ScaleMode::Major => match degree {
                    1 | 4 | 5 => ChordQuality::Major,
                    2 | 3 | 6 => ChordQuality::Minor,
                    7 => ChordQuality::Diminished,
                    _ => ChordQuality::Major,
                },
                ScaleMode::Minor => match degree {
                    1 | 4 => ChordQuality::Minor,
                    2 => ChordQuality::Diminished,
                    3 | 6 | 7 => ChordQuality::Major,
                    5 => ChordQuality::Minor,
                    _ => ChordQuality::Minor,
                },
                _ => ChordQuality::Major,
            }
        });

        Self::new(root, quality)
    }
}

/// Get scale notes in an octave
pub fn scale_notes(root: u8, mode: ScaleMode) -> Vec<u8> {
    mode.intervals()
        .iter()
        .map(|&interval| (root + interval).min(127))
        .collect()
}

/// Quantize a note to the nearest scale note
pub fn quantize_to_scale(note: u8, root: u8, mode: ScaleMode) -> u8 {
    let scale = mode.intervals();
    let note_in_octave = note % 12;
    let root_in_octave = root % 12;
    let octave = note / 12;

    // Find relative position to root
    let relative = (note_in_octave + 12 - root_in_octave) % 12;

    // Find nearest scale degree
    let mut min_dist = 12u8;
    let mut nearest = 0u8;

    for &interval in scale {
        let dist = if interval > relative {
            (interval - relative).min(relative + 12 - interval)
        } else {
            (relative - interval).min(interval + 12 - relative)
        };

        if dist < min_dist {
            min_dist = dist;
            nearest = interval;
        }
    }

    let quantized_in_octave = (root_in_octave + nearest) % 12;
    (octave * 12 + quantized_in_octave).min(127)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_euclidean_rhythm() {
        // Classic patterns
        assert_eq!(euclidean_rhythm(8, 3, 0), vec![true, false, false, true, false, false, true, false]);
        assert_eq!(euclidean_rhythm(8, 5, 0), vec![true, false, true, true, false, true, true, false]);
        assert_eq!(euclidean_rhythm(16, 4, 0), vec![true, false, false, false, true, false, false, false, true, false, false, false, true, false, false, false]);
    }

    #[test]
    fn test_chord_generator() {
        let chord = ChordGenerator::new(60, ChordQuality::Major);
        assert_eq!(chord.generate(), vec![60, 64, 67]); // C major
    }

    #[test]
    fn test_quantize_to_scale() {
        // C major scale: C D E F G A B (0, 2, 4, 5, 7, 9, 11)
        // When equidistant, algorithm picks first scale degree found
        assert_eq!(quantize_to_scale(61, 60, ScaleMode::Major), 60); // C# -> C (equidist C/D)
        assert_eq!(quantize_to_scale(63, 60, ScaleMode::Major), 62); // D# -> D (equidist D/E)
        assert_eq!(quantize_to_scale(66, 60, ScaleMode::Major), 65); // F# -> F (equidist F/G)
        assert_eq!(quantize_to_scale(60, 60, ScaleMode::Major), 60); // C stays C
        assert_eq!(quantize_to_scale(64, 60, ScaleMode::Major), 64); // E stays E
    }
}
