# agent-rubato

> *Rubato* — Italian for "stolen time." In music, it's the expressive practice of
> borrowing time from slow passages to spend on fast ones, keeping the overall
> pulse alive while allowing flexibility. This crate brings that same philosophy
> to agent scheduling.

**Tempo flexibility for adaptive agent scheduling.**

## Overview

Agent systems often run at a fixed cadence — poll every 30 seconds, check every
minute, process at constant throughput. But real work isn't metronomic. Sometimes
an agent needs to surge (accelerando), sometimes it needs to ease off
(ritardando), and sometimes it just needs to breathe (rubato).

`agent-rubato` provides the building blocks for tempo-aware agent scheduling:

- **TempoMarking** — Define a target BPM (beats per minute) with a "human feel"
  factor that introduces natural variance around the target pace. No agent should
  be a metronome.

- **RubatoProfile** — Set boundaries on how much tempo can stretch or compress.
  A strict profile keeps the agent close to the target. An expressive profile
  allows wide tempo fluctuations. The agent borrows time from slow phases and
  spends it during bursts.

- **TempoCurve** — Program smooth tempo transitions: accelerando (speeding up),
  ritardando (slowing down), or free rubato. Uses ease-in-ease-out interpolation
  for natural-feeling transitions.

- **TempoFollower** — Track another agent's tempo by observing their actions.
  The follower estimates BPM from observed timestamps and computes sync offset
  so it can adjust its own pace to match.

- **TempoLeader** — Broadcast tempo to a group of agents. Set tempo, start
  curves, manage followers with individual offsets, and enable group rubato.

- **TempoMap** — Plan tempo changes across an entire session. Add tempo entries
  at specific beat positions, look up the BPM at any point, and compute total
  session duration.

## When to Use This

- **Adaptive scheduling** — Your agent's workload varies and fixed intervals
  waste resources during quiet periods or cause backpressure during spikes.
- **Coordinated agents** — Multiple agents need to work in tempo together,
  with one leading and others following.
- **Expressive pacing** — You want your agent to feel responsive and alive
  rather than mechanical and predictable.
- **Session planning** — You need to map out tempo changes across a long-running
  task (slow warmup → fast work → cool-down).

## Quick Start

```rust
use agent_rubato::{TempoMarking, RubatoProfile, TempoCurve, TempoLeader, TempoFollower};

// Define the agent's base tempo with some human feel
let tempo = TempoMarking::labeled(120.0, 0.3, "Working pace");

// Allow up to 20% tempo variation
let profile = RubatoProfile::moderate();

// Program an accelerando: start slow, speed up over 16 beats
let curve = TempoCurve::new(80.0, 160.0, 16).with_label("warmup phase");

// Set up a leader to coordinate agents
let mut leader = TempoLeader::new("orchestrator", tempo);
leader.add_follower("worker-1", 0.0);
leader.add_follower("worker-2", 10.0);
leader.start_curve(curve);

// Follow another agent's tempo
let mut follower = TempoFollower::new("orchestrator");
follower.observe_beat();
// ... after observing several beats:
// let estimated = follower.estimated_bpm();
```

## Core Concepts

### BPM as Agent Frequency

In this crate, BPM (beats per minute) maps directly to how frequently an agent
acts. A BPM of 120 means 2 actions per second. A BPM of 60 means one action
per second. The concept translates naturally.

### Human Feel

Robotic timing is detectable and often undesirable. The `feel` factor on
`TempoMarking` introduces controlled variance around the target BPM, making
agent behavior feel more natural and less predictable.

### Rubato: Stolen Time

The core insight: agents don't need constant tempo. During slow phases, they
can "save" time budget. During busy phases, they can "spend" it. A
`RubatoProfile` defines the bounds of this flexibility.

## License

MIT
