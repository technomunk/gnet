# GNet protocol

This document describes the networking protocol GNet uses over UDP for synchronizing messages.

## Overview

GNet provides a simple way of sending small atomic messages reliably in an efficient message. It
does so in a manner inspired by Gaffer on Games
[networking article](https://www.gafferongames.com/tags/networking/). The core principle is to
provide any received data to the application as soon as it arrives, even if some data is missing
in between. GNet provides virtual `Connections` over
[UDP](https://en.wikipedia.org/wiki/User_Datagram_Protocol) that can send and receive `Packages`,
a binary-serializeable data type. The `Packages` are application-specific data serialized version
of which does not exceed a certain maximum size (typically 1KiB). The `Connections` also provide a
binary stream for larger messages, however it is less efficient than `Packages` (because of
additional order guarantees) or `TCP` (which has the benefit of lower-level implementation as well
as hardware implementations).

## Terminology

- **Packet** - collection of bytes sent across network. Fixed size for a particular application,
however the exact size is application defined.
- **Package** - an application defined data type that can be serialized and deserialized into a
small number of bytes (smaller than **packet** size) that is used to transmit application data with
reduced delay.
- **Stream** - a contiguous collection of bytes synchronized across network. Used to transmit
application data that can't be transmitted using **packages**.
- **Endpoint** - one of 2 ends of a **connection** that is responsible for sending and receiving
**packets** across network.
- **Connection** - a virtual link between 2 **endpoints**.
- **Client** - an **endpoint** that initializes a **connection** by requesting it from **server**.
A typical use-case has multiple clients and a single **server**, however in a
[P2P](https://en.wikipedia.org/wiki/Peer-to-peer) setup a single application can be both a
**server** and a **client**.
- **Server** - an **endpoint** that listens for incoming **connections** from potential **clients**.
- **Listener** - a specialized **endpoint** for **servers** that can receive incoming requests for
connections and either *accept*, *deny* or *ignore* them.
- **Address** - a unique identifier of an **endpoint** on the network.

## Protocol

### Establishing a connection

In order to establish a connection a **server** and at least a single **client** is required. The
**server** begins listening for incoming connections by opening a `ConnectionListener`. Once the
**server listener** is established a **client** may begin establishing a connection by invoking
`Connection::connect()` with the **server's** address. This creates a `PendingConnection`, which
initializes the [*establishing handshake*](#establishing-handshake). If the requested connection
is accepted by the server the `PendingConnection` may be promoted to a fully functional
`Connection` by using `try_promote()`. Once the client-side `Connection` is promoted it may be
used to synchronize data between the **server** and the **client**. The `PendingConnection` will
continue attempting to perform the [*establishing handshake*](#establishing-handshake) until it
receives a rejection from the **server** or timeout period is reached, at which point any calls
to `try_promote()` are guaranteed to return appropriate errors.

### Establishing handshake

A **client** generates a random *handshake id* and sends a `connection_request` packet with
payload supplied from the application. Upon receiving the request, the `ConnectionListener`
remembers the *handshake id* and associates a *connection id* with it, creating a new
`Connection` that may be used by the **server**. The `ConnectionListener` also sends a
`connection_accept` packet, which includes new client id and has the same *handshake id* as the
request. The listener will repeatedly answer with `connection_accept` upon receiving duplicate
`connection_request` with the same *handshake id* as the accepted request, as long as the
`Connection` with the resulting id is live on the **server** side.

### Transmitting data

Application data is transmitted through 2 mechanisms: **packages** and **streams**.
<!-- TODO: explain the difference and their benefits -->

## Packet anatomy

GNet uses [User Datagram Protocol](https://en.wikipedia.org/wiki/User_Datagram_Protocol) with
statically sized packets.

Packets consist of a header and payload, the header has following structure:

- **Hash** (4 bytes) : a safety checksum seeded with an application-specific secret.
- **Connection id** (2 bytes) : a unique identifier for connection (session) between 2 endpoints.
- **Packet id** (1 byte) : unique identifier of this network packet.
- **Acknowledged packet id** (1 byte) : unique identifier of the latest (largest) acknowledged
network packet by the other endpoint.
- **Acknowledged packet mask** (8 bytes) : individual bits representing previous 64 received packets.
- **Signal** (4 bytes) : signalling bitpatterns.
- **Data prelude** (4 bytes) : application data specific to a network packet.

Reliable packets get assigned a numeric sequence id, which uniquely identifies them. Up to 65
reliable packets may be in-flight (in unacknowledged state) at once to avoid over-complicating
deduplication logic. Packets deemed lost are simply re-sent as-is.

Packets are deemed lost if:

- Their acknowledgement has not been received for 2xRTT time.
- Their acknowledgement has not been received, but acknowledgements for 8 subsequent packets have.
