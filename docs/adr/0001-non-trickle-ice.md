# ADR-0001: Non-Trickle ICE

## Status

Accepted

## Context

WebRTC supports two ICE gathering modes:

- **Trickle ICE**: ICE candidates are sent as they're discovered, allowing parallel SDP exchange and candidate gathering. Modern browsers and most WebRTC libraries default to this.
- **Non-Trickle ICE (Vanilla ICE)**: All candidates are gathered before the SDP offer/answer is created. The complete candidate list is embedded in the SDP.

blnk uses non-trickle ICE. The signaling server rejects trickle candidates with an explicit error.

## Decision

Use non-trickle ICE for all peer connections.

## Consequences

**Positive**:

- **Simpler state machine**: No need to handle mid-signaling candidate additions, race conditions between candidate arrival and SDP exchange, or partial candidate sets.
- **Deterministic signaling flow**: Offer → Answer → Connect is strictly sequential. No out-of-band candidate messages to multiplex.
- **Easier testing**: `TwoPeerHarness` can create fully-formed offers synchronously without waiting for candidate gathering.
- **BitBang compatibility**: The upstream BitBang protocol uses vanilla ICE. Non-trickle maintains wire compatibility.

**Negative**:

- **Slower connection setup**: Candidate gathering (STUN/TURN probing) blocks SDP creation. Adds 500ms-2s to connection time depending on network.
- **Not browser-native**: Browser WebRTC APIs default to trickle. Any future browser-based peer would need adaptation.
- ** TURN candidate delay**: TURN server probing is the slowest part of gathering; non-trickle forces it to complete before any signaling begins.

**Neutral**:

- LAN connections (host candidates) gather instantly. The penalty only affects WAN/TURN paths.
- The signaling protocol is already designed for this: `OfferMessage.streams` carries the full candidate set in-band.

## Alternatives Considered

1. **Trickle ICE**: Would require a new `IceCandidateMessage` type, handling for candidate-arrival races, and a state machine that tolerates partial SDP. Rejected for complexity.
2. **Hybrid (trickle for WAN, vanilla for LAN)**: Adds mode detection complexity. Rejected for minimal gain — LAN is already fast.
