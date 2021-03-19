# GNet overview

The GNet library provides tools and utilities for creating effective networking on top of UDP.
The library provides:

- [Data reliability](#data-reliability).

## Data reliability

GNet provides a mechanism for delivering data reliably. It is done by assigning sent datagrams a
sequential id and including acknowledgements for received packets with given ids in newly sent
datagrams. This is done with **PacketTracker**. This allows to detect and retransmitted datagrams
that were not delivered successfully. The user-application has full access to individual datagrams
fascilitating 

## Questions

- Should GNet force a header?
- When to retransmit?
- Estimating RTT.
