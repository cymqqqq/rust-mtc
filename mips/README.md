# MIPs

MIPs stand for **MTC Improvement Proposal**.

They exist to document what may be implemented by [MTC](https://github.com/cymqqqq/rust-mtc)- any compatible canister development kit.

---

- [List](#list)
- [Message Kinds](#event-kinds)
- [Message Types](#message-types)
  - [Client to Relay](#client-to-relay)
  - [Relay to Client](#relay-to-client)
- [Standardized Tags](#standardized-tags)

---

## List

- [NIP-01: Basic protocol flow description](01.md)
- [NIP-02: Follow List](Coming soon)
- [NIP-03: OpenTimestamps Attestations for Events](Coming soon)
- [NIP-04: Encrypted Direct Message](Coming soon)deprecated in favor of 
## Message Kinds

| kind          | description                     | MIP                                    |
| ------------- | ------------------------------- | -------------------------------------- |
| `0`           | User Metadata                   | [01](01.md)                            |
| `1`           | Short Text Note                 | [01](01.md)                            |


### Client to Relay

| type    | description                                         | NIP         |
| ------- | --------------------------------------------------- | ----------- |
| `EVENT` | used to publish events                              | [01](01.md) |
| `REQ`   | used to request events and subscribe to new updates | [01](01.md) |
| `CLOSE` | used to stop previous subscriptions                 | [01](01.md) |
| `AUTH`  | used to send authentication events                  | [42](42.md) |
| `COUNT` | used to request event counts                        | [45](45.md) |

### Relay to Client

| type     | description                                             | NIP         |
| -------- | ------------------------------------------------------- | ----------- |
| `EOSE`   | used to notify clients all stored events have been sent | [01](01.md) |
| `EVENT`  | used to send events requested to clients                | [01](01.md) |
| `NOTICE` | used to send human-readable messages to clients         | [01](01.md) |
| `OK`     | used to notify clients if an EVENT was successful       | [01](01.md) |
| `CLOSED` | used to notify clients that a REQ was ended and why     | [01](01.md) |
| `AUTH`   | used to send authentication challenges                  | [42](42.md) |
| `COUNT`  | used to send requested event counts to clients          | [45](45.md) |

## Standardized Tags

| name              | value                                | other parameters                | NIP                                   |
| ----------------- | ------------------------------------ | ------------------------------- | ------------------------------------- |
| `m`               | message id (hex)                       | relay URL, marker, pubkey (hex) | [01](01.md)           |
| `p`               | pubkey (hex)                         | relay URL, nickname              | [01](01.md), [02](02.md)              |

Please update these lists when proposing new MIPs.

