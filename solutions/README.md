# Solutions

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


