# MIPs

MIPs stand for **MTC Improvement Proposal**.

They exist to document what may be implemented by [MTC](https://github.com/cymqqqq/rust-mtc)- any compatible canister development kit.

---

- [List](#list)
- [Message Kinds](#MESSAGE-kinds)
- [Message Types](#message-types)
  - [Client to Relay](#client-to-relay)
  - [Relay to Client](#relay-to-client)
- [Standardized Tags](#standardized-tags)

---

## List

- [MIP-01: Basic protocol flow description](01.md)

## Message Kinds

| kind          | description                     | MIP                                    |
| ------------- | ------------------------------- | -------------------------------------- |
| `0`           | User Metadata                   | [01](01.md)                            |
| `1`           | Short Text Note                 | [01](01.md)                            |


### Client to Relay

| type    | description                                         | MIP         |
| ------- | --------------------------------------------------- | ----------- |
| `MESSAGE` | used to publish MESSAGEs                              | [01](01.md) |
| `REQ`   | used to request MESSAGEs and subscribe to new updates | [01](01.md) |
| `CLOSE` | used to stop previous subscriptions                 | [01](01.md) |

### Relay to Client

| type     | description                                             | MIP         |
| -------- | ------------------------------------------------------- | ----------- |
| `EOSE`   | used to notify clients all stored MESSAGEs have been sent | [01](01.md) |
| `MESSAGE`  | used to send MESSAGEs requested to clients                | [01](01.md) |
| `NOTICE` | used to send human-readable messages to clients         | [01](01.md) |
| `OK`     | used to notify clients if an MESSAGE was successful       | [01](01.md) |
| `CLOSED` | used to notify clients that a REQ was ended and why     | [01](01.md) |


## Standardized Tags

| name              | value                                | other parameters                | MIP                                   |
| ----------------- | ------------------------------------ | ------------------------------- | ------------------------------------- |
| `m`               | message id (hex)                       | relay URL, marker, pubkey (hex) | [01](01.md)           |
| `p`               | pubkey (hex)                         | relay URL, nickname              | [01](01.md)             |

Please update these lists when proposing new MIPs.

