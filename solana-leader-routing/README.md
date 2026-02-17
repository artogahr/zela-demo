# solana-leader-routing

A Zela procedure that determines which server region should handle a request
based on the geographic location of the current Solana leader.

## How it works

1. Queries Solana mainnet for the current slot and its leader via `getSlot` + `getSlotLeaders`
2. Looks up the leader's pubkey in a precomputed geo map (bundled in the binary)
3. Maps the leader's geographic location to the closest server region

### Region mapping

| Leader location | Closest region |
|---|---|
| Europe, Africa | Frankfurt |
| North America, South America | NewYork |
| Asia, Oceania | Tokyo |
| Middle East, South Asia | Dubai |

If the leader is not in the geo map, `leader_geo` is set to `"UNKNOWN"` and
`closest_region` defaults to `Frankfurt` (where the majority of Solana validators
are located).

## Building

The procedure is built automatically by Zela when pushed to the repository.
To check it locally:

```
cargo check -p solana-leader-routing
cargo test -p solana-leader-routing
```

## Calling the procedure

Obtain a JWT and call the executor with the procedure name and commit hash:

```
curl --header "Authorization: Bearer $TOKEN" \
     --header "Content-Type: application/json" \
     --data '{"jsonrpc":"2.0","id":1,"method":"zela.solana-leader-router#<commit_hash>","params":null}' \
     https://executor.zela.io
```

Or using the helper script:

```
ZELA_PROJECT_KEY_ID=<key_id> ZELA_PROJECT_KEY_SECRET=<key_secret> \
  ./run-procedure.sh solana-leader-router#<commit_hash> 'null'
```

### Sample response

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "slot": 400954435,
    "leader": "DZv25oNCWFvGXu9tH63BiAXvG94syweGZhbvdN3HxDxT",
    "leader_geo": "Asia",
    "closest_region": "Tokyo"
  }
}
```

## Geo map generation

The geo map is generated offline by the `geo-generator` tool and bundled into
the procedure at compile time via `include_str!`.

To regenerate:

```
IPINFO_TOKEN=<your_token> cargo run -p geo-generator > solana-leader-routing/src/geo_map.json
```

This does the following:
1. Calls `getClusterNodes` on Solana mainnet to get validator pubkeys and gossip IPs
2. Queries ipinfo.io/lite for each unique IP to get continent and country
3. Maps each validator to a coarse geo label (Europe, North America, Asia, etc.)
4. Writes a JSON map of `{pubkey: geo_label}` to stdout

The resulting file is ~300KB for ~5000 validators, well within the ~10MB target.

## Assumptions and failure modes

**Assumptions:**
- Validator locations don't change frequently. Datacenter migrations are rare
  and planned events. The geo map can be regenerated periodically (e.g. monthly)
  without issues.
- `getClusterNodes` gossip addresses reflect where validators actually run.
  In practice some validators use proxies or relays, so the IP may not be their
  true location. For coarse continent-level mapping this is acceptable.

**Failure modes:**
- If `getSlot` or `getSlotLeaders` fail, the procedure returns an error with
  a descriptive message.
- If a leader isn't in the geo map (new validator, or one that wasn't in the
  cluster at generation time), we return `leader_geo: "UNKNOWN"` and default
  to Frankfurt.

**Region flapping:**
The leader schedule is deterministic for an entire epoch (~2-3 days). For any
given slot, the leader is always the same validator. Flapping can only occur
at slot boundaries when consecutive leaders are in different regions — this is
inherent to the problem and not something we should suppress (the whole point
is to route to the nearest region). We use `confirmed` commitment to avoid
chasing unconfirmed slots.