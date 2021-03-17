# GNet protocol

This document describes the networking protocol GNet uses over UDP for synchronizing messages.

## Overview

GNet provides a simple way of sending small atomic messages reliably in an efficient message. It
does so in a manner inspired by Gaffer on Games
[networking article](https://www.gafferongames.com/tags/networking/). The core principle is to
provide any received data to the application as soon as it arrives, even if some data is missing
in between. GNet provides virtual `Connections` over
[UDP](https://en.wikipedia.org/wiki/User_Datagram_Protocol) that can send and receive `Messages`,
a binary-serializeable data type. The `Message` are application-specific data serialized version
of which does not exceed a certain maximum size (typically 1KiB). The `Connections` also provide a
binary stream for larger messages, however it is less efficient than `Messages` (because of
additional order guarantees) or `TCP` (which has the benefit of lower-level implementation as well
as hardware implementations).

## Terminology

- **Address** - a unique identifier of an **endpoint** on the network.
- **Datagram** - sequence of bytes sent across network. Has an associated source and destination
addresses.
- **Parcel** - a datagram that includes a GNet header.
- **Message** - an application defined data type that can be serialized and deserialized into a
small number of bytes (smaller than **parcel** size) that is used to transmit application data with
reduced delay.
- **Stream** - a contiguous collection of bytes synchronized across network. Used to transmit
application data that can't be transmitted using **Messages**.
- **Endpoint** - one of 2 ends of a **connection** that is responsible for sending and receiving
**parcels** across network.
- **Connection** - a virtual link between 2 **endpoints**.
- **Client** - an **endpoint** that initializes a **connection** by requesting it from **server**.
A typical use-case has multiple clients and a single **server**, however in a
[P2P](https://en.wikipedia.org/wiki/Peer-to-peer) setup a single application can be both a
**server** and a **client**.
- **Server** - an **endpoint** that listens for incoming **connections** from potential **clients**.
- **Listener** - a specialized **endpoint** for **servers** that can receive incoming requests for
connections and either *accept*, *deny* or *ignore* them.

## Parcel anatomy

GNet uses [User Datagram Protocol](https://en.wikipedia.org/wiki/User_Datagram_Protocol).

### Header

Each GNet parcel starts with 1 byte **signal** bitmask. The signalling bits are as follows:

0. **Connection** - whether the parcel is associated with an existent connection. If set the
 **signal** is followed by a *connection id*. If unset the **signal** is followed by
 *handshake id*.
1. **Answer | Indexed** - if **connection** bit is set signals whether the parcel is *indexed*. If
set the **signal** is followed by packet id. Non-indexed parcels may not be acknowledged and are
thus only delivered in the best-effort manner. If **connection** bit is unset signals whether the
parcel is a new connection request or an answer to one. The parcel is an answer to a connection
request parcel if set.
2. **Accept | Acknowledge** - if **connection** bit is set signals whether the parcel contains
**ack mask**. If **connection** bit is not set signals whether the connection requested was
accepted, if set the **signal** is followed by **connection id**.
3. **Message** - the parcel contains some **message** bytes. If set the **signal** is followed by
*message length*. Requires **connection** bit to be set.
4. **Stream** - the parcel contains application **stream** slice. If set the **signal** is followed
by *stream length*. Requires **connection** bit to be set.
5. **RESERVED 0** - reserved for future use by GNet. Must be unset.
6. **RESERVED 1** - reserved for future use by GNet. Must be unset.
7. **Parity** - parity bit for the **signal**. The whole bitmask must have odd parity. IE the
**parity** bit should be set if there are even number of other set bits.

Structure:

- **Signal** (1 byte) : bitmask that signals how the following bytes should be interpreted.
- **Connection id | Handshake id** (2 bytes) : unique identifier of the parcel context.
- **Packet index** (optional 1 byte) : identifying number of the packet.
- **Ack mask** (optional 9 bytes) : identification of received parcel indices by the other end of the
connection.
- **Message length** (optional 2 bytes) : number of bytes that contain application **messages**.
- **Stream length** (optional 2 bytes) : number of bytes that contain application data **stream** slice.

Packets are deemed lost if:

- Their acknowledgement has not been received for 2xRTT time.
- Their acknowledgement has not been received, but acknowledgements for 8 subsequent packets have.

## Establishing a connection

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

## Establishing handshake

A **client** generates a random *handshake id* and sends a `connection_request` parcel with
payload supplied from the application. Upon receiving the request, the `ConnectionListener`
remembers the *handshake id* and associates a *connection id* with it, creating a new
`Connection` that may be used by the **server**. The `ConnectionListener` also sends a
`connection_accept` parcel, which includes new client id and has the same *handshake id* as the
request. The listener will repeatedly answer with `connection_accept` upon receiving duplicate
`connection_request` with the same *handshake id* as the accepted request, as long as the
`Connection` with the resulting id is live on the **server** side.

## Transmitting data

Application data is transmitted through 2 mechanisms: **messages** and **streams**.
<!-- TODO: explain the difference and their benefits -->
