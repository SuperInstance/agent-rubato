//! # agent-rubato
//!
//! Tempo flexibility for adaptive agent scheduling.
//!
//! In music, *rubato* means "stolen time" — the performer borrows time from
//! slow passages to spend on fast ones, keeping the overall pulse alive while
//! allowing expressive flexibility. This crate brings that same idea to agent
//! scheduling: agents can stretch and compress their work timing adaptively,
//! borrowing cycles from idle phases and spending them during bursts.

use std::collections::HashMap;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// TempoMarking
// ---------------------------------------------------------------------------

/// A tempo marking with BPM and a "human feel" factor.
///
/// Rather than rigid metronomic timing, `TempoMarking` introduces slight
/// variance around the target BPM, simulating how human musicians (and
/// well-tuned agents) naturally ebb and flow around a target pace.
#[derive(Debug, Clone, PartialEq)]
pub struct TempoMarking {
    /// Base beats per minute.
    bpm: f64,
    /// Human-feel variance factor (0.0 = robotic, 1.0 = very expressive).
    feel: f64,
    /// Optional label (e.g. "Allegro", "urgent", "idle").
    label: Option<String>,
}

impl TempoMarking {
    /// Create a new tempo marking.
    pub fn new(bpm: f64, feel: f64) -> Self {
        Self {
            bpm: bpm.clamp(1.0, 600.0),
            feel: feel.clamp(0.0, 1.0),
            label: None,
        }
    }

    /// Create a tempo marking with a human-readable label.
    pub fn labeled(bpm: f64, feel: f64, label: impl Into<String>) -> Self {
        Self {
            bpm: bpm.clamp(1.0, 600.0),
            feel: feel.clamp(0.0, 1.0),
            label: Some(label.into()),
        }
    }

    /// The base BPM.
    pub fn bpm(&self) -> f64 {
        self.bpm
    }

    /// The human-feel factor.
    pub fn feel(&self) -> f64 {
        self.feel
    }

    /// Duration of one beat at the base BPM.
    pub fn beat_duration(&self) -> Duration {
        Duration::from_secs_f64(60.0 / self.bpm)
    }

    /// Duration of one beat with feel applied (± variance).
    /// The `variation` parameter should be in [-1.0, 1.0] and is scaled by feel.
    pub fn beat_with_feel(&self, variation: f64) -> Duration {
        let base_secs = 60.0 / self.bpm;
        let adjusted = base_secs * (1.0 + variation * self.feel * 0.1);
        Duration::from_secs_f64(adjusted.max(0.001))
    }

    /// The label, if any.
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// Well-known tempo: Largo (slow, broad).
    pub fn largo() -> Self {
        Self::labeled(50.0, 0.3, "Largo")
    }

    /// Well-known tempo: Andante (walking pace).
    pub fn andante() -> Self {
        Self::labeled(80.0, 0.2, "Andante")
    }

    /// Well-known tempo: Allegro (fast, lively).
    pub fn allegro() -> Self {
        Self::labeled(140.0, 0.15, "Allegro")
    }

    /// Well-known tempo: Presto (very fast).
    pub fn presto() -> Self {
        Self::labeled(180.0, 0.1, "Presto")
    }
}

// ---------------------------------------------------------------------------
// RubatoProfile
// ---------------------------------------------------------------------------

/// Describes how much tempo can stretch for an agent.
///
/// A `RubatoProfile` sets the boundaries of permissible timing flexibility.
/// An agent with high stretch can borrow a lot of time; one with low stretch
/// must stay close to the written tempo.
#[derive(Debug, Clone, PartialEq)]
pub struct RubatoProfile {
    /// Maximum percentage the tempo can increase (compressed, faster).
    max_compress: f64,
    /// Maximum percentage the tempo can decrease (stretched, slower).
    max_stretch: f64,
    /// How quickly the tempo can change (rate per second).
    transition_rate: f64,
    /// Name of the profile.
    name: String,
}

impl RubatoProfile {
    /// Create a new rubato profile.
    pub fn new(name: impl Into<String>, max_compress: f64, max_stretch: f64, transition_rate: f64) -> Self {
        Self {
            name: name.into(),
            max_compress: max_compress.clamp(0.0, 1.0),
            max_stretch: max_stretch.clamp(0.0, 1.0),
            transition_rate: transition_rate.clamp(0.0, 10.0),
        }
    }

    /// Strict profile: minimal tempo flexibility.
    pub fn strict() -> Self {
        Self::new("strict", 0.05, 0.05, 0.1)
    }

    /// Moderate profile: balanced flexibility.
    pub fn moderate() -> Self {
        Self::new("moderate", 0.2, 0.25, 0.5)
    }

    /// Expressive profile: wide tempo flexibility.
    pub fn expressive() -> Self {
        Self::new("expressive", 0.4, 0.5, 1.0)
    }

    /// Apply rubato to a base BPM, given a stretch factor.
    ///
    /// `factor` ranges from -1.0 (max stretch/slow) to +1.0 (max compress/fast).
    pub fn apply(&self, base_bpm: f64, factor: f64) -> f64 {
        let clamped = factor.clamp(-1.0, 1.0);
        if clamped >= 0.0 {
            base_bpm * (1.0 + clamped * self.max_compress)
        } else {
            base_bpm * (1.0 - clamped.abs() * self.max_stretch)
        }
    }

    /// Compute the minimum BPM allowed by this profile.
    pub fn min_bpm(&self, base_bpm: f64) -> f64 {
        base_bpm * (1.0 - self.max_stretch)
    }

    /// Compute the maximum BPM allowed by this profile.
    pub fn max_bpm(&self, base_bpm: f64) -> f64 {
        base_bpm * (1.0 + self.max_compress)
    }

    /// Whether a given BPM is within the rubato range for a base BPM.
    pub fn is_within_range(&self, base_bpm: f64, actual_bpm: f64) -> bool {
        actual_bpm >= self.min_bpm(base_bpm) && actual_bpm <= self.max_bpm(base_bpm)
    }

    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn max_compress(&self) -> f64 {
        self.max_compress
    }
    pub fn max_stretch(&self) -> f64 {
        self.max_stretch
    }
    pub fn transition_rate(&self) -> f64 {
        self.transition_rate
    }
}

// ---------------------------------------------------------------------------
// TempoCurve
// ---------------------------------------------------------------------------

/// Shape of a tempo change over time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CurveShape {
    /// Gradually speeding up.
    Accelerando,
    /// Gradually slowing down.
    Ritardando,
    /// Free rubato (expressive fluctuation).
    Rubato,
    /// Constant tempo (no change).
    Flat,
}

/// A tempo curve: a programmed change in tempo over a duration.
///
/// Like a musician's accelerando or ritardando, a `TempoCurve` smoothly
/// transitions from one tempo to another over a given time span.
#[derive(Debug, Clone)]
pub struct TempoCurve {
    /// The starting tempo (BPM).
    start_bpm: f64,
    /// The ending tempo (BPM).
    end_bpm: f64,
    /// Duration of the curve in beats.
    duration_beats: u32,
    /// Shape of the curve.
    shape: CurveShape,
    /// Optional label.
    label: Option<String>,
}

impl TempoCurve {
    /// Create a new tempo curve.
    pub fn new(start_bpm: f64, end_bpm: f64, duration_beats: u32) -> Self {
        let shape = if (start_bpm - end_bpm).abs() < 0.01 {
            CurveShape::Flat
        } else if end_bpm > start_bpm {
            CurveShape::Accelerando
        } else {
            CurveShape::Ritardando
        };
        Self {
            start_bpm,
            end_bpm,
            duration_beats: duration_beats.max(1),
            shape,
            label: None,
        }
    }

    /// Create a rubato-shaped curve (free fluctuation).
    pub fn rubato(base_bpm: f64, fluctuation: f64, duration_beats: u32) -> Self {
        Self {
            start_bpm: base_bpm,
            end_bpm: base_bpm + fluctuation,
            duration_beats: duration_beats.max(1),
            shape: CurveShape::Rubato,
            label: None,
        }
    }

    /// Label the curve.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Get the BPM at a given beat position (linear interpolation).
    pub fn bpm_at_beat(&self, beat: f64) -> f64 {
        let t = (beat / self.duration_beats as f64).clamp(0.0, 1.0);
        self.start_bpm + (self.end_bpm - self.start_bpm) * t
    }

    /// Get the BPM at a given normalized position (0.0 = start, 1.0 = end).
    /// Uses ease-in-ease-out for smoother transitions.
    pub fn bpm_at_position(&self, pos: f64) -> f64 {
        let t = pos.clamp(0.0, 1.0);
        // Ease-in-ease-out: 3t² - 2t³
        let eased = if self.shape == CurveShape::Flat {
            t
        } else {
            3.0 * t * t - 2.0 * t * t * t
        };
        self.start_bpm + (self.end_bpm - self.start_bpm) * eased
    }

    /// The shape of the curve.
    pub fn shape(&self) -> CurveShape {
        self.shape
    }

    /// Start BPM.
    pub fn start_bpm(&self) -> f64 {
        self.start_bpm
    }

    /// End BPM.
    pub fn end_bpm(&self) -> f64 {
        self.end_bpm
    }

    /// Duration in beats.
    pub fn duration_beats(&self) -> u32 {
        self.duration_beats
    }

    /// Label.
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// Total change in BPM.
    pub fn delta(&self) -> f64 {
        self.end_bpm - self.start_bpm
    }

    /// Is the curve speeding up?
    pub fn is_accelerando(&self) -> bool {
        self.shape == CurveShape::Accelerando
    }

    /// Is the curve slowing down?
    pub fn is_ritardando(&self) -> bool {
        self.shape == CurveShape::Ritardando
    }
}

// ---------------------------------------------------------------------------
// TempoFollower
// ---------------------------------------------------------------------------

/// Track the tempo of other agents and follow along.
///
/// A `TempoFollower` observes timestamps of another agent's actions and
/// infers their current tempo, allowing this agent to synchronize.
#[derive(Debug, Clone)]
pub struct TempoFollower {
    /// The agent being followed.
    leader_id: String,
    /// Recent beat timestamps.
    observed_beats: Vec<Instant>,
    /// Maximum beats to track.
    window_size: usize,
    /// Smoothing factor for BPM estimation (0.0–1.0).
    smoothing: f64,
    /// Current estimated BPM of the leader.
    estimated_bpm: Option<f64>,
}

impl TempoFollower {
    /// Create a new follower for the given leader.
    pub fn new(leader_id: impl Into<String>) -> Self {
        Self {
            leader_id: leader_id.into(),
            observed_beats: Vec::new(),
            window_size: 16,
            smoothing: 0.3,
            estimated_bpm: None,
        }
    }

    /// Set the observation window size.
    pub fn with_window_size(mut self, size: usize) -> Self {
        self.window_size = size.max(2);
        self
    }

    /// Set the smoothing factor.
    pub fn with_smoothing(mut self, smoothing: f64) -> Self {
        self.smoothing = smoothing.clamp(0.0, 1.0);
        self
    }

    /// Observe a beat from the leader.
    pub fn observe_beat(&mut self) {
        let now = Instant::now();
        self.observed_beats.push(now);
        if self.observed_beats.len() > self.window_size {
            self.observed_beats.remove(0);
        }
        self.recalculate();
    }

    /// Observe a beat at a specific time (for testing).
    pub fn observe_beat_at(&mut self, time: Instant) {
        self.observed_beats.push(time);
        if self.observed_beats.len() > self.window_size {
            self.observed_beats.remove(0);
        }
        self.recalculate();
    }

    fn recalculate(&mut self) {
        if self.observed_beats.len() < 2 {
            return;
        }
        let first = self.observed_beats[0];
        let last = *self.observed_beats.last().unwrap();
        let elapsed = last.duration_since(first).as_secs_f64();
        if elapsed <= 0.0 {
            return;
        }
        let intervals = self.observed_beats.len() - 1;
        let raw_bpm = (intervals as f64 / elapsed) * 60.0;
        self.estimated_bpm = Some(match self.estimated_bpm {
            Some(prev) => prev + self.smoothing * (raw_bpm - prev),
            None => raw_bpm,
        });
    }

    /// Get the estimated BPM of the leader.
    pub fn estimated_bpm(&self) -> Option<f64> {
        self.estimated_bpm
    }

    /// The leader being followed.
    pub fn leader_id(&self) -> &str {
        &self.leader_id
    }

    /// Number of observed beats.
    pub fn observed_count(&self) -> usize {
        self.observed_beats.len()
    }

    /// Compute the sync offset: how far off our tempo is from the leader's.
    pub fn sync_offset(&self, our_bpm: f64) -> Option<f64> {
        self.estimated_bpm.map(|leader| leader - our_bpm)
    }

    /// Reset all observations.
    pub fn reset(&mut self) {
        self.observed_beats.clear();
        self.estimated_bpm = None;
    }
}

// ---------------------------------------------------------------------------
// TempoLeader
// ---------------------------------------------------------------------------

/// Set tempo for a group of agents.
///
/// A `TempoLeader` broadcasts tempo information so that followers can
/// synchronize their work cycles.
#[derive(Debug, Clone)]
pub struct TempoLeader {
    /// The leader's identifier.
    leader_id: String,
    /// Current tempo.
    current: TempoMarking,
    /// Registered followers.
    followers: HashMap<String, f64>,
    /// Whether rubato is enabled for the group.
    rubato_enabled: bool,
    /// Active tempo curve, if any.
    active_curve: Option<TempoCurve>,
    /// Current position in the curve (0.0–1.0).
    curve_position: f64,
}

impl TempoLeader {
    /// Create a new tempo leader.
    pub fn new(leader_id: impl Into<String>, initial_tempo: TempoMarking) -> Self {
        Self {
            leader_id: leader_id.into(),
            current: initial_tempo,
            followers: HashMap::new(),
            rubato_enabled: false,
            active_curve: None,
            curve_position: 0.0,
        }
    }

    /// Enable or disable rubato for the group.
    pub fn set_rubato(&mut self, enabled: bool) {
        self.rubato_enabled = enabled;
    }

    /// Register a follower with an optional tempo offset.
    pub fn add_follower(&mut self, follower_id: impl Into<String>, offset_bpm: f64) {
        self.followers.insert(follower_id.into(), offset_bpm);
    }

    /// Remove a follower.
    pub fn remove_follower(&mut self, follower_id: &str) -> bool {
        self.followers.remove(follower_id).is_some()
    }

    /// Change the tempo.
    pub fn set_tempo(&mut self, tempo: TempoMarking) {
        self.current = tempo;
    }

    /// Start a tempo curve (accelerando/ritardando).
    pub fn start_curve(&mut self, curve: TempoCurve) {
        self.active_curve = Some(curve);
        self.curve_position = 0.0;
    }

    /// Advance the curve by a step (0.0–1.0 increment).
    /// Returns the current BPM from the curve.
    pub fn advance_curve(&mut self, step: f64) -> Option<f64> {
        if let Some(ref curve) = self.active_curve {
            self.curve_position = (self.curve_position + step).min(1.0);
            let bpm = curve.bpm_at_position(self.curve_position);
            if self.curve_position >= 1.0 {
                self.current = TempoMarking::new(bpm, self.current.feel);
                self.active_curve = None;
            }
            Some(bpm)
        } else {
            None
        }
    }

    /// Get the effective BPM, accounting for any active curve.
    pub fn effective_bpm(&self) -> f64 {
        if let Some(ref curve) = self.active_curve {
            curve.bpm_at_position(self.curve_position)
        } else {
            self.current.bpm()
        }
    }

    /// Get the tempo for a specific follower (base ± offset).
    pub fn tempo_for_follower(&self, follower_id: &str) -> f64 {
        let base = self.effective_bpm();
        let offset = self.followers.get(follower_id).copied().unwrap_or(0.0);
        base + offset
    }

    /// Leader ID.
    pub fn leader_id(&self) -> &str {
        &self.leader_id
    }

    /// Number of followers.
    pub fn follower_count(&self) -> usize {
        self.followers.len()
    }

    /// Current tempo marking.
    pub fn current_tempo(&self) -> &TempoMarking {
        &self.current
    }

    /// Whether rubato is enabled.
    pub fn rubato_enabled(&self) -> bool {
        self.rubato_enabled
    }

    /// Whether a curve is active.
    pub fn curve_active(&self) -> bool {
        self.active_curve.is_some()
    }
}

// ---------------------------------------------------------------------------
// TempoMap
// ---------------------------------------------------------------------------

/// A map of tempo changes over a session.
///
/// A `TempoMap` stores planned tempo curves and markings across a timeline,
/// letting agents look up what tempo they should be at during any point
/// in a session.
#[derive(Debug, Clone)]
pub struct TempoMapEntry {
    /// Start beat of this entry.
    start_beat: f64,
    /// The tempo or curve at this point.
    bpm: f64,
    /// Optional label.
    label: Option<String>,
}

/// Tempo map: tempo changes across a session.
#[derive(Debug, Clone)]
pub struct TempoMap {
    entries: Vec<TempoMapEntry>,
    total_beats: f64,
    default_bpm: f64,
}

impl TempoMap {
    /// Create a new tempo map with a default BPM.
    pub fn new(default_bpm: f64) -> Self {
        Self {
            entries: Vec::new(),
            total_beats: 0.0,
            default_bpm,
        }
    }

    /// Add a tempo marking at a given beat.
    pub fn add_marking(&mut self, beat: f64, bpm: f64, label: Option<String>) {
        let entry = TempoMapEntry {
            start_beat: beat,
            bpm,
            label,
        };
        self.entries.push(entry);
        self.entries.sort_by(|a, b| a.start_beat.partial_cmp(&b.start_beat).unwrap());
        if beat > self.total_beats {
            self.total_beats = beat;
        }
    }

    /// Look up the BPM at a given beat position.
    pub fn bpm_at_beat(&self, beat: f64) -> f64 {
        if self.entries.is_empty() {
            return self.default_bpm;
        }
        // Find the last entry at or before this beat.
        let mut result = self.default_bpm;
        for entry in &self.entries {
            if entry.start_beat <= beat {
                result = entry.bpm;
            } else {
                break;
            }
        }
        result
    }

    /// Total beats in the map.
    pub fn total_beats(&self) -> f64 {
        self.total_beats
    }

    /// Number of tempo entries.
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Whether the map is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get all entries.
    pub fn entries(&self) -> &[TempoMapEntry] {
        &self.entries
    }

    /// Compute the total duration of the map at the given tempos.
    pub fn total_duration(&self) -> Duration {
        if self.entries.is_empty() {
            return Duration::from_secs(0);
        }
        let mut total = 0.0_f64;
        for i in 0..self.entries.len() {
            let start = self.entries[i].start_beat;
            let end = if i + 1 < self.entries.len() {
                self.entries[i + 1].start_beat
            } else {
                self.total_beats
            };
            let beats = end - start;
            let secs_per_beat = 60.0 / self.entries[i].bpm;
            total += beats * secs_per_beat;
        }
        Duration::from_secs_f64(total)
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // --- TempoMarking tests ---

    #[test]
    fn test_tempo_marking_creation() {
        let tm = TempoMarking::new(120.0, 0.5);
        assert_eq!(tm.bpm(), 120.0);
        assert_eq!(tm.feel(), 0.5);
        assert!(tm.label().is_none());
    }

    #[test]
    fn test_tempo_marking_labeled() {
        let tm = TempoMarking::labeled(100.0, 0.2, "Moderato");
        assert_eq!(tm.label(), Some("Moderato"));
    }

    #[test]
    fn test_tempo_marking_clamping() {
        let tm = TempoMarking::new(0.0, 5.0);
        assert_eq!(tm.bpm(), 1.0);
        assert_eq!(tm.feel(), 1.0);
    }

    #[test]
    fn test_beat_duration() {
        let tm = TempoMarking::new(60.0, 0.0);
        assert_eq!(tm.beat_duration(), Duration::from_secs(1));
        let tm2 = TempoMarking::new(120.0, 0.0);
        assert_eq!(tm2.beat_duration(), Duration::from_millis(500));
    }

    #[test]
    fn test_beat_with_feel() {
        let robotic = TempoMarking::new(100.0, 0.0);
        let base = robotic.beat_duration();
        let with_zero_feel = robotic.beat_with_feel(1.0);
        assert_eq!(base, with_zero_feel); // no feel = no variation

        let human = TempoMarking::new(100.0, 1.0);
        let with_feel = human.beat_with_feel(1.0);
        assert_ne!(base, with_feel);
    }

    #[test]
    fn test_well_known_tempos() {
        assert_eq!(TempoMarking::largo().bpm(), 50.0);
        assert_eq!(TempoMarking::andante().bpm(), 80.0);
        assert_eq!(TempoMarking::allegro().bpm(), 140.0);
        assert_eq!(TempoMarking::presto().bpm(), 180.0);
    }

    // --- RubatoProfile tests ---

    #[test]
    fn test_rubato_profile_creation() {
        let p = RubatoProfile::new("test", 0.1, 0.2, 0.5);
        assert_eq!(p.name(), "test");
        assert_eq!(p.max_compress(), 0.1);
        assert_eq!(p.max_stretch(), 0.2);
        assert_eq!(p.transition_rate(), 0.5);
    }

    #[test]
    fn test_rubato_apply_compress() {
        let p = RubatoProfile::moderate();
        let result = p.apply(100.0, 1.0); // max compress
        assert!(result > 100.0);
        assert_eq!(result, 120.0); // 100 * (1 + 0.2)
    }

    #[test]
    fn test_rubato_apply_stretch() {
        let p = RubatoProfile::moderate();
        let result = p.apply(100.0, -1.0); // max stretch
        assert_eq!(result, 75.0); // 100 * (1 - 0.25)
    }

    #[test]
    fn test_rubato_apply_neutral() {
        let p = RubatoProfile::moderate();
        let result = p.apply(100.0, 0.0);
        assert_eq!(result, 100.0);
    }

    #[test]
    fn test_rubato_range() {
        let p = RubatoProfile::expressive();
        assert_eq!(p.min_bpm(100.0), 50.0);  // 100 * 0.5
        assert_eq!(p.max_bpm(100.0), 140.0);  // 100 * 1.4
        assert!(p.is_within_range(100.0, 80.0));
        assert!(p.is_within_range(100.0, 120.0));
        assert!(!p.is_within_range(100.0, 30.0));
    }

    #[test]
    fn test_preset_profiles() {
        let s = RubatoProfile::strict();
        assert!(s.max_compress() < 0.1);
        let m = RubatoProfile::moderate();
        assert!(m.max_compress() > s.max_compress());
        let e = RubatoProfile::expressive();
        assert!(e.max_compress() > m.max_compress());
    }

    // --- TempoCurve tests ---

    #[test]
    fn test_accelerando_curve() {
        let c = TempoCurve::new(80.0, 140.0, 16);
        assert_eq!(c.shape(), CurveShape::Accelerando);
        assert!(c.is_accelerando());
        assert!(!c.is_ritardando());
    }

    #[test]
    fn test_ritardando_curve() {
        let c = TempoCurve::new(140.0, 80.0, 16);
        assert_eq!(c.shape(), CurveShape::Ritardando);
        assert!(c.is_ritardando());
    }

    #[test]
    fn test_flat_curve() {
        let c = TempoCurve::new(120.0, 120.0, 8);
        assert_eq!(c.shape(), CurveShape::Flat);
    }

    #[test]
    fn test_curve_bpm_at_beat() {
        let c = TempoCurve::new(100.0, 200.0, 10);
        assert_eq!(c.bpm_at_beat(0.0), 100.0);
        assert_eq!(c.bpm_at_beat(5.0), 150.0);
        assert_eq!(c.bpm_at_beat(10.0), 200.0);
    }

    #[test]
    fn test_curve_bpm_at_position() {
        let c = TempoCurve::new(100.0, 200.0, 10);
        assert_eq!(c.bpm_at_position(0.0), 100.0);
        assert_eq!(c.bpm_at_position(1.0), 200.0);
        // Ease-in-ease-out: midpoint should be exactly 150.0 for linear-like curve
        let mid = c.bpm_at_position(0.5);
        assert_eq!(mid, 150.0); // 3*(0.25) - 2*(0.125) = 0.5 => 100 + 100*0.5 = 150
    }

    #[test]
    fn test_curve_with_label() {
        let c = TempoCurve::new(60.0, 120.0, 8).with_label("crescendo section");
        assert_eq!(c.label(), Some("crescendo section"));
    }

    #[test]
    fn test_curve_delta() {
        let c = TempoCurve::new(80.0, 140.0, 8);
        assert_eq!(c.delta(), 60.0);
    }

    #[test]
    fn test_rubato_curve() {
        let c = TempoCurve::rubato(120.0, 10.0, 32);
        assert_eq!(c.shape(), CurveShape::Rubato);
        assert_eq!(c.start_bpm(), 120.0);
        assert_eq!(c.end_bpm(), 130.0);
    }

    // --- TempoFollower tests ---

    #[test]
    fn test_follower_creation() {
        let f = TempoFollower::new("leader-1");
        assert_eq!(f.leader_id(), "leader-1");
        assert!(f.estimated_bpm().is_none());
        assert_eq!(f.observed_count(), 0);
    }

    #[test]
    fn test_follower_estimates_bpm() {
        let mut f = TempoFollower::new("leader");
        let now = Instant::now();
        // Simulate 4 beats at 120 BPM (0.5s apart)
        for i in 0..5 {
            f.observe_beat_at(now + Duration::from_millis(500 * i as u64));
        }
        let bpm = f.estimated_bpm().unwrap();
        // Should be approximately 120 BPM
        assert!(bpm > 110.0 && bpm < 130.0, "Expected ~120 BPM, got {}", bpm);
    }

    #[test]
    fn test_follower_sync_offset() {
        let mut f = TempoFollower::new("leader");
        let now = Instant::now();
        for i in 0..5 {
            f.observe_beat_at(now + Duration::from_millis(500 * i as u64));
        }
        let offset = f.sync_offset(100.0).unwrap();
        assert!(offset > 0.0); // Leader is faster than us
    }

    #[test]
    fn test_follower_reset() {
        let mut f = TempoFollower::new("leader");
        f.observe_beat();
        f.observe_beat();
        f.reset();
        assert_eq!(f.observed_count(), 0);
        assert!(f.estimated_bpm().is_none());
    }

    #[test]
    fn test_follower_window_size() {
        let mut f = TempoFollower::new("leader").with_window_size(4);
        for _ in 0..10 {
            f.observe_beat();
        }
        assert_eq!(f.observed_count(), 4);
    }

    // --- TempoLeader tests ---

    #[test]
    fn test_leader_creation() {
        let l = TempoLeader::new("conductor", TempoMarking::allegro());
        assert_eq!(l.leader_id(), "conductor");
        assert_eq!(l.follower_count(), 0);
        assert!(!l.rubato_enabled());
        assert!(!l.curve_active());
    }

    #[test]
    fn test_leader_followers() {
        let mut l = TempoLeader::new("conductor", TempoMarking::allegro());
        l.add_follower("agent-a", 0.0);
        l.add_follower("agent-b", 10.0);
        assert_eq!(l.follower_count(), 2);
        assert_eq!(l.tempo_for_follower("agent-a"), 140.0);
        assert_eq!(l.tempo_for_follower("agent-b"), 150.0);
        assert_eq!(l.tempo_for_follower("unknown"), 140.0); // default offset = 0
    }

    #[test]
    fn test_leader_remove_follower() {
        let mut l = TempoLeader::new("conductor", TempoMarking::andante());
        l.add_follower("agent-a", 0.0);
        assert!(l.remove_follower("agent-a"));
        assert!(!l.remove_follower("agent-a"));
        assert_eq!(l.follower_count(), 0);
    }

    #[test]
    fn test_leader_set_tempo() {
        let mut l = TempoLeader::new("conductor", TempoMarking::andante());
        l.set_tempo(TempoMarking::presto());
        assert_eq!(l.effective_bpm(), 180.0);
    }

    #[test]
    fn test_leader_rubato() {
        let mut l = TempoLeader::new("conductor", TempoMarking::allegro());
        l.set_rubato(true);
        assert!(l.rubato_enabled());
    }

    #[test]
    fn test_leader_curve() {
        let mut l = TempoLeader::new("conductor", TempoMarking::new(80.0, 0.0));
        let curve = TempoCurve::new(80.0, 140.0, 16);
        l.start_curve(curve);
        assert!(l.curve_active());

        // Advance through the curve
        let bpm_25 = l.advance_curve(0.25);
        assert!(bpm_25.is_some());
        assert!(l.curve_active());

        let bpm_50 = l.advance_curve(0.25);
        assert!(bpm_50.is_some());

        let bpm_100 = l.advance_curve(0.50);
        assert!(bpm_100.is_some());
        assert!(!l.curve_active()); // curve complete
        assert_eq!(l.effective_bpm(), bpm_100.unwrap());
    }

    // --- TempoMap tests ---

    #[test]
    fn test_empty_tempo_map() {
        let map = TempoMap::new(120.0);
        assert!(map.is_empty());
        assert_eq!(map.bpm_at_beat(0.0), 120.0);
        assert_eq!(map.bpm_at_beat(100.0), 120.0);
    }

    #[test]
    fn test_tempo_map_entries() {
        let mut map = TempoMap::new(120.0);
        map.add_marking(0.0, 80.0, Some("Intro".into()));
        map.add_marking(16.0, 120.0, Some("Theme".into()));
        map.add_marking(48.0, 100.0, Some("Bridge".into()));
        map.add_marking(64.0, 140.0, Some("Finale".into()));

        assert_eq!(map.entry_count(), 4);
        assert_eq!(map.bpm_at_beat(0.0), 80.0);
        assert_eq!(map.bpm_at_beat(15.0), 80.0);
        assert_eq!(map.bpm_at_beat(16.0), 120.0);
        assert_eq!(map.bpm_at_beat(32.0), 120.0);
        assert_eq!(map.bpm_at_beat(48.0), 100.0);
        assert_eq!(map.bpm_at_beat(60.0), 100.0);
        assert_eq!(map.bpm_at_beat(64.0), 140.0);
    }

    #[test]
    fn test_tempo_map_out_of_range() {
        let mut map = TempoMap::new(120.0);
        map.add_marking(0.0, 100.0, None);
        map.add_marking(8.0, 120.0, None);
        // Beyond the last entry, use the last entry's BPM
        assert_eq!(map.bpm_at_beat(100.0), 120.0);
    }

    #[test]
    fn test_tempo_map_default() {
        let map = TempoMap::new(90.0);
        assert_eq!(map.bpm_at_beat(0.0), 90.0);
    }

    #[test]
    fn test_tempo_map_unordered_insert() {
        let mut map = TempoMap::new(120.0);
        map.add_marking(32.0, 140.0, None);
        map.add_marking(0.0, 80.0, None);
        map.add_marking(16.0, 120.0, None);
        // Entries should be sorted by beat
        assert_eq!(map.bpm_at_beat(0.0), 80.0);
        assert_eq!(map.bpm_at_beat(16.0), 120.0);
        assert_eq!(map.bpm_at_beat(32.0), 140.0);
    }

    #[test]
    fn test_tempo_map_total_beats() {
        let mut map = TempoMap::new(120.0);
        map.add_marking(0.0, 100.0, None);
        map.add_marking(64.0, 120.0, None);
        assert_eq!(map.total_beats(), 64.0);
    }

    #[test]
    fn test_tempo_map_duration() {
        let mut map = TempoMap::new(120.0);
        // 8 beats at 120 BPM = 4 seconds
        map.add_marking(0.0, 120.0, None);
        map.total_beats = 8.0;
        let dur = map.total_duration();
        assert_eq!(dur, Duration::from_secs(4));
    }

    #[test]
    fn test_tempo_map_entries_access() {
        let mut map = TempoMap::new(120.0);
        map.add_marking(0.0, 80.0, Some("A".into()));
        map.add_marking(16.0, 120.0, Some("B".into()));
        let entries = map.entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].label.as_deref(), Some("A"));
        assert_eq!(entries[1].label.as_deref(), Some("B"));
    }
}
