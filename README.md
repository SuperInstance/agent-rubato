# agent-rubato

> *Rubato* — Italian for "stolen time." In music, it's the expressive practice of
> borrowing time from slow passages to spend on fast ones, keeping the overall
> pulse alive while allowing flexibility. This crate brings that same philosophy
> to agent scheduling.

**agent-rubato** is a Rust library implementing adaptive tempo control for multi-agent systems. It provides tempo markings (BPM with human-feel variance), rubato profiles (per-agent stretch/compress boundaries), tempo curves (accelerando/ritardando transitions), leader-follower synchronization, and session-level tempo maps — applying musical expressiveness to computational scheduling.

## Why It Matters

Rigid metronomic scheduling is suboptimal for agents operating in dynamic environments. When traffic spikes, an agent should compress its cycle (run faster); during idle periods, it should stretch (run slower, save resources). This is not just rate-limiting — it is *expressive* timing that preserves the global rhythm while allowing local flexibility, exactly as a musician shapes a phrase within a steady beat.

The musical analogy maps precisely to agent systems:

- **Tempo (BPM)** → Agent cycle frequency (actions per minute)
- **Rubato profile** → Per-agent timing flexibility budget (SLA boundaries)
- **Accelerando** → Scale-up ramp when load increases
- **Ritardando** → Cool-down ramp when load decreases
- **Tempo curve** → Programmed transition between operating modes
- **Leader/follower** → Coordinator/worker synchronization pattern
- **Tempo map** → Session-level schedule of planned tempo changes

In distributed systems, this models **adaptive polling intervals**, **exponential backoff with jitter**, and **workload-aware scheduling** — but with a unified, composable API rooted in five centuries of music theory.

## How It Works

### Tempo Markings with Human Feel

`TempoMarking` defines a target BPM plus a *feel* factor (0.0 = robotic, 1.0 = very expressive). The `beat_with_feel(variation)` method applies bounded jitter:

```
T_actual = T_base × (1 + variation × feel × 0.1)
```

Where `variation ∈ [-1.0, +1.0]` is a per-beat random variable. With feel = 0, timing is deterministic. With feel = 1.0, each beat varies by ±10% — comparable to human musicians' natural tempo fluctuation (Repp, 2005: ±6–12% for expressive performances).

**Big-O:** `beat_with_feel` is O(1) — a single multiply and add.

### Rubato Profiles

A `RubatoProfile` constrains how far an agent can deviate from its base tempo:

```
BPM_allowed ∈ [base × (1 - max_stretch), base × (1 + max_compress)]
```

The `apply(base_bpm, factor)` method computes the actual BPM given a stretch factor ∈ [-1, +1]:

- `factor = +1.0` → maximum compression (fastest allowed)
- `factor = -1.0` → maximum stretch (slowest allowed)
- `factor = 0.0` → exactly base tempo

Three presets cover common agent profiles:

| Profile | max_compress | max_stretch | transition_rate | Use case |
|---------|-------------|-------------|-----------------|----------|
| Strict | 5% | 5% | 0.1/s | Hard real-time (fixed deadlines) |
| Moderate | 20% | 25% | 0.5/s | General-purpose adaptive scheduling |
| Expressive | 40% | 50% | 1.0/s | Best-effort batch processing |

The `transition_rate` limits how quickly BPM can change per second, preventing oscillation:

```
ΔBPM/Δt ≤ transition_rate × BPM_base
```

### Tempo Curves (Accelerando / Ritardando)

`TempoCurve` defines a smooth transition from `start_bpm` to `end_bpm` over `duration_beats`. The interpolation uses an **ease-in-ease-out** function (smoothstep):

```
f(t) = 3t² - 2t³   (Hermite smoothstep)
BPM(t) = start + (end - start) × f(t)
```

This produces S-shaped transitions: slow start, fast middle, slow end — matching how human performers execute tempo changes. Linear interpolation (used by `bpm_at_beat`) is available for programmatic access.

**Curve shapes:**

- `Accelerando`: end_bpm > start_bpm (speeding up)
- `Ritardando`: end_bpm < start_bpm (slowing down)
- `Flat`: end_bpm ≈ start_bpm (no change)
- `Rubato`: free fluctuation around a base

### Leader-Follower Synchronization

`TempoFollower` observes a leader's beat timestamps and estimates their BPM using an **exponentially weighted moving average (EWMA)**:

```
BPM_est(t) = BPM_est(t-1) + α × (BPM_raw - BPM_est(t-1))
```

Where `α = smoothing ∈ [0, 1]`. With α = 0.3 (default), 30% of the estimate comes from the latest measurement, providing stability against single-beat outliers while tracking genuine tempo changes.

The follower maintains a sliding window of the last N observations (default 16) and computes raw BPM as:

```
BPM_raw = (N-1) / (t_last - t_first) × 60
```

The `sync_offset(our_bpm)` method returns the delta between leader and follower tempo, enabling proportional correction (analogous to a PLL phase detector).

**Big-O:** O(1) per observation (amortized, ring buffer), O(W) for recalculation where W = window_size.

### Tempo Leader

`TempoLeader` broadcasts tempo to a group of followers, with per-follower offsets. It manages active tempo curves and advances them incrementally:

```
position(t+1) = position(t) + step
```

When `position ≥ 1.0`, the curve completes and the leader adopts the curve's end BPM as its new base. Followers query `tempo_for_follower(id)` to get their assigned tempo (base ± offset).

### Tempo Map

`TempoMap` stores tempo markings across a session timeline. At query time, `bpm_at_beat(beat)` returns the marking at or immediately before the query beat — an O(log N) binary search (O(N) in the current linear implementation, acceptable for small N).

## Quick Start

```rust
use agent_rubato::*;

// Standard tempo at 120 BPM with moderate feel
let tempo = TempoMarking::new(120.0, 0.3);
assert_eq!(tempo.beat_duration(), std::time::Duration::from_millis(500));

// Apply rubato profile
let profile = RubatoProfile::moderate();
let compressed = profile.apply(120.0, 1.0);  // max compress
assert_eq!(compressed, 144.0);               // 120 × 1.20

// Define a tempo curve (accelerando over 16 beats)
let curve = TempoCurve::new(80.0, 140.0, 16).with_label("ramp up");
assert_eq!(curve.bpm_at_beat(0.0), 80.0);
assert_eq!(curve.bpm_at_beat(16.0), 140.0);
assert!(curve.is_accelerando());

// Leader-follower synchronization
let mut leader = TempoLeader::new("conductor", TempoMarking::allegro());
leader.add_follower("agent-a", 0.0);
leader.add_follower("agent-b", 10.0); // offset +10 BPM
assert_eq!(leader.tempo_for_follower("agent-b"), 150.0);

// Session tempo map
let mut map = TempoMap::new(120.0);
map.add_marking(0.0, 80.0, Some("Intro".into()));
map.add_marking(16.0, 120.0, Some("Theme".into()));
map.add_marking(64.0, 140.0, Some("Finale".into()));
assert_eq!(map.bpm_at_beat(32.0), 120.0);
```

## API

| Type | Key Methods | Description |
|------|-------------|-------------|
| `TempoMarking` | `new`, `beat_duration`, `beat_with_feel`, `largo`/`andante`/`allegro`/`presto` | BPM + human-feel variance |
| `RubatoProfile` | `new`, `strict`/`moderate`/`expressive`, `apply`, `min_bpm`, `max_bpm` | Timing flexibility boundaries |
| `TempoCurve` | `new`, `rubato`, `bpm_at_beat`, `bpm_at_position` | Programmed tempo transition |
| `TempoFollower` | `new`, `observe_beat`, `observe_beat_at`, `estimated_bpm`, `sync_offset` | Leader tempo estimation |
| `TempoLeader` | `new`, `set_tempo`, `start_curve`, `advance_curve`, `add_follower` | Group conductor |
| `TempoMap` | `new`, `add_marking`, `bpm_at_beat`, `total_duration` | Session-level tempo schedule |

## Architecture Notes

Agent-rubato provides the **temporal coordination layer** for the Lau ecosystem. Within γ + η = C, tempo is a conserved resource: if one agent speeds up (γ increases), the followers' tempo must adjust to maintain the system's overall beat coherence (η responds). The conservation invariant is the session's total beat count — a tempo curve that compresses time in one section must compensate by stretching another, exactly as in musical rubato where "borrowed" time must be "returned."

The leader-follower pattern instantiates the γ/η boundary: the leader's tempo is the agent contribution (γ), the followers' synchronization error is the environmental response (η), and their sum is the system's coherent rhythm (C).

See the [architecture overview](https://github.com/SuperInstance/agent-rubato/blob/main/ARCHITECTURE.md).

## References

1. Repp, B.H. (2005). "Expressive timing in Schumann's 'Träumerei': A window on the creative process?" *Music Perception*, 23(1), 3–25.
2. Honing, H. (2001). "From time to time: The representation of timing and tempo." *Computer Music Journal*, 25(3), 50–61.
3. Todd, N.P.M. (1985). "A Model of Expressive Timing in Tonal Music." *Music Perception*, 3(1), 33–57.
4. Bharucha, J.J. (1996). "Melodic Anchoring." *Music Perception*, 13(3), 383–400.

## License

MIT
