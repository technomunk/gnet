# GNet protocol

This document describes the networking protocol GNet uses over UDP for synchronizing messages.

## Packets

GNet uses [User Datagram Protocol](https://en.wikipedia.org/wiki/User_Datagram_Protocol) with statically sized messages (currently 1048 bytes).

Packets consist of a header and payload, the header has following structure:

- Hash (4 bytes) : a safety checksum seeded with an application-specific secret.
- Connection Id (2 bytes) : a unique identifier for connection (session) between 2 endpoints.
- Packet Id (1 byte) : unique identifier of this network packet.
- Acknowledged packet id (1 byte) : unique identifier of the latest (largest) acknowledged network packet by the other endpoint.
- Acknowledged packet mask (8 bytes) : individual bits representing previous 64 received packets.
- Signal (4 bytes) : signalling bitpatterns.
- Data prelude (4 bytes) : application data specific to a network packet.

Reliable packets get assigned a numeric sequence id, which uniquely identifies them.
Up to 65 reliable packets may be in-flight (in unacknowledged state) at once to avoid over-complicating deduplication logic.
Packets deemed lost are simply re-sent as-is.

Packets are deemed lost if:

- Their acknowledgement has not been received for 2xRTT time.
- Their acknowledgement has not been received, but acknowledgements for 8 contiguous subsequent packets have.
