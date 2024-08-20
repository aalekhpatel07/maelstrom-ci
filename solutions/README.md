# Solutions

These are my solutions for some of the [distributed systems challenges](https://fly.io/dist-sys/) by [fly.io](https://fly.io/).

## (Common Setup)

- [`solutions::io::io_channel`](https://github.com/aalekhpatel07/maelstrom-ci/blob/main/solutions/src/io.rs) provides a (sender, receiver) pair of channels that can be used to transport any (de)serializable messages to/from stdout/stdin.

- [`solutions::message`](https://github.com/aalekhpatel07/maelstrom-ci/blob/main/solutions/src/message.rs) contains some utility structs that help setup the envelope and metadata around payloads to instruct the Maelstrom routing system where a payload is coming from and where it is headed.

## Echo

[Solution](https://github.com/aalekhpatel07/maelstrom-ci/blob/8fe2d3881023c68250f750e268b5989aee106b04/solutions/src/bin/echo.rs)

#### Explanation

There's nothing to do, really. Just echo back what we get. This challenge is really just an excuse to test out the rest of the Maelstrom related scaffolding in this crate.

## Unique ID Generation

[Solution](https://github.com/aalekhpatel07/maelstrom-ci/blob/f601b893bca9ca07b3d484196b184b296d5f4204/solutions/src/bin/unique_id_generation.rs)

#### Explanation

Send ids of the form `<node_id>_<random_usize>_<atomic_counter>` where 
the `random_usize` is a random int of either `32` or `64` bytes that is generated once when the node boots up, and the atomic counter starts at `0` when the node boots up. This takes care of ensuring uniqueness in the face of network partitions and recoveries.

## Broadcast

### Single Node Broadcast

[Solution](https://github.com/aalekhpatel07/maelstrom-ci/blob/7be56506627358bb9b50a972a2fba2f9ae5f0bc3/solutions/src/bin/broadcast.rs)

#### Explanation
Pretty straightforward implementation that doesn't need any inter-node communication at all since there is just one node. Just follow along the RPCs while capturing the messages in a local state.

### Multi Node Broadcast

[Solution](https://github.com/aalekhpatel07/maelstrom-ci/blob/1dca4c844e808c23a550c496b05558d74c54e680/solutions/src/bin/broadcast.rs)

#### Explanation

Now we gotta propagate the messages to our neighbors, but it might be inefficient to do it one message at a time. So buffer a buncha messages, and send them out periodically, without worrying about acknowledgement because we are assuming it is guaranteed to reach the nodes (since there is no parition or weird communication errors).

### Fault Tolerant Broadcast 

[Solution](https://github.com/aalekhpatel07/maelstrom-ci/blob/d042da34ff3514c8e6dec2e33eee76d5cae9a7fd/solutions/src/bin/broadcast.rs)

#### Explanation

Not only do we have to propagate the messages, but we also need to store a cache of unacknowledged messages so a delivery can be retried later. The cache should be updated every time we get an acknowledgment of messages from a peer.

### Efficient Broadcast (Parts 1 and 2)

[Solution](https://github.com/aalekhpatel07/maelstrom-ci/blob/9dc2845378184aee2c94a17cf27d1e91ae304e9e/solutions/src/bin/broadcast.rs)

#### Explanation

This challenge brings performance requirements on the network traffic for internal messages, and the amount of time it takes for the broadcasts to propagate across the entire cluster. We perform a few optimizations to achieve this:

- Only propagate messages to neighbors when a node sees them for the first time.
- Define a custom topology for the cluster where each node is connected to at most `NODE_COUNT / STRIDE` other nodes.
- Due to the added latency (of `100ms`) for successful message deliveries, we choose to wait a little bit longer before attempting to sync the messages with our neighbors.
- We define the `STRIDE` knob to minimize the maximum number of hops needed for a message to replicate across the cluster. The smaller the value (`> 1`), the more neighbors a node has, and therefore more messages will flow into the network but potentially less forwards would be necessary. The higher the value (`< NODE_COUNT`), the less neighbors a node has, and therefore fewer messages will flow into the network but potentially more forwards (i.e. hops) would be necessary for a succesful replication across the cluster.
- We also define a `TICK_RATE_MS` knob, to control how often should locally buffered messages be synchronized amongst a node's neighbors. The higher the value, the smaller the network traffic but also larger latencies. The smaller the value, the larger the network traffic but also smaller latencies since messages are synced faster.

- For the first part of the challenge (i.e. `3d)`), we set `STRIDE=3` and `TICK_RATE_MS=155` and achieve the following target:

```
messages per operation ~ 29.1 (<= 30)
median latency ~ 395ms (<= 400ms)
maximum latency ~ 475ms (<= 600ms)
```

- For the second part of the challenge (i.e. `3e)`), we set `STRIDE=4` and `TICK_RATE_MS=250` and achieve the following target:

```
messages per operation ~ 15 (<= 20)
median latency ~ 772ms (<= 1s)
maximum latency ~ 901ms (<= 2s)
```
