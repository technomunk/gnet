# GNet protocol

This document describes the networking protocol GNet uses over UDP for synchronizing messages.

## Overview

GNet provides a simple way of sending small atomic messages reliably in an efficient message.
It does so in a manner inspired by Gaffer on Games [networking article](https://www.gafferongames.com/tags/networking/).
The core principle is to provide any received data to the application as soon as it arrives, even if some data is missing in between.
GNet provides virtual `Connections` over [UDP](https://en.wikipedia.org/wiki/User_Datagram_Protocol) that can send and receive `Packages`, a binary-serializeable data type.
The `Packages` are application-specific data serialized version of which does not exceed a certain maximum size (typically 1KiB).
The `Connections` also provide a binary stream for larger messages, however it is less efficient than `Packages` (because of additional order guarantees) or `TCP` (which has the benefit of lower-level implementation as well as hardware implementations).

## Terminology

- **Packet** - collection of bytes sent across network. Fixed size for a particular application, however the exact size is application defined.
- **Package** - an application defined data type that can be serialized and deserialized into a small number of bytes (smaller than **packet** size) that is used to transmit application data with reduced delay.
- **Stream** - a contiguous collection of bytes synchronized across network. Used to transmit application data that can't be transmitted using **packages**.
- **Endpoint** - one of 2 ends of a **connection** that is responsible for sending and receiving **packets** across network.
- **Connection** - a virtual link between 2 **endpoints**.
- **Client** - an **endpoint** that initializes a **connection** by requesting it from **server**. A typical use-case has multiple clients and a single **server**, however in a [P2P](https://en.wikipedia.org/wiki/Peer-to-peer) setup a single application can be both a **server** and a **client**.
- **Server** - an **endpoint** that listens for incoming **connections** from potential **clients**.
- **Listener** - a specialized **endpoint** for **servers** that can receive incoming requests for connections and either *accept*, *deny* or *ignore* them.
- **Address** - a unique identifier of an **endpoint** on the network.

## Protocol

### Establishing a connection

In order to establish a connection a **server** and at least a single **client** is required.
The **server** begins listening for incoming connections by constructing a `Listener`.
Once the **server listener** is established a **client** may begin establishing a connection by calling `Connection::connect()` with the **server's** address.
This creates a `PendingConnection`, which can repeatedly send a *connection request packet* to the server.
If a *connection request packet* makes it to the **server**, it can then by processed by the **listener**.
The **listener** can *accept*, *deny* or *ignore* the request.
*Accepted* requests immediately create a `Connection` on the **server** and allow transmission of data from the server.
The new `Connection` will transmit packets to the **client**, which will be able promote its `PendingConnection` to a full `Connection`, once those packets begin arriving.
The **listener** may instead *deny* the connection, which will result in sending a single **packet** to the requesting client, explicitly denying the connection.
It may also simply *ignore* the request, saving outgoing network traffic, however reducing the client's responsiveness.
The `PendingConnection` will be deemed failed if no answer is received within a *timeout* period or an explicit denial is received.

### Transmitting data

Application data is transmitted through 2 mechanisms: **packages** and **streams**.
<!-- TODO: explain the difference and their benefits -->

## Packet anatomy

GNet uses [User Datagram Protocol](https://en.wikipedia.org/wiki/User_Datagram_Protocol) with statically sized packets.

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
