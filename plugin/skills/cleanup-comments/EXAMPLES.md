# Worked Examples

Each case shows a comment as found and what it SHOULD become.

## Restatement

```python
# Before
# increment the retry counter
retries += 1

# After
retries += 1
```

"Because increment the retry counter" is incoherent and fails the restatement test, so it is
deleted.

## Change Narration

```typescript
// Before
// updated to use the v2 client and now also handles null
const user = await client.fetchUser(id)

// After
const user = await client.fetchUser(id)
```

A year from now, no reader will understand the code better for knowing this. Delete.

## Bloated Reason

```go
// Before
// This is the first key/value pair of every emitted line. It gives every event the
// stable prefix {"stream":"bi.telemetry", which means the events are filterable in
// CloudWatch Logs Insights using a filter on stream, and can also be used as a
// subscription filter pattern later on when the Snowpipe landing (DT-2485) gets
// built out by the data team.
line := fmt.Sprintf(`{"stream":%q,`, streamName)

// After
// Stable first-key prefix; CloudWatch Logs Insights filters on it.
line := fmt.Sprintf(`{"stream":%q,`, streamName)
```

The comment carries a real constraint and states it at four times the length it needs. The first
sentence states what the code shows. The second sentence opens with the real information, a stable
prefix that CloudWatch Logs Insights can filter on, and then trails into a second use for that
prefix and unbuilt future work. Only the opening of the second sentence survives.

## Protected Comment

```python
# Keep
# pence, not pounds
DEFAULT_FEE = 250
```

This looks like a restatement but isn't. If it were deleted, the constant's unit would be lost. The
comment MUST be retained.

## Cross-File Sync Pointer

```typescript
// Keep
// Keep in sync with the status labels in components/StatusBadge.tsx
const STATUS_LABELS = { open: 'Open', closed: 'Closed' }
```

The pointer is a valuable warning to future editors. The comment MUST be retained.

## Doc Comment: Over-Applied

```python
# Before
def _normalize_slug(value: str) -> str:
    """Normalize the slug.

    Args:
        value: The value to normalize.

    Returns:
        The normalized slug.
    """

# After
def _normalize_slug(value: str) -> str:
```

The function is an internal helper in a file where sibling functions do not have doc comments. The
audience step fails: no caller uses this without reading it. The density step fails: no sibling has
one. Moreover, every sentence merely restates what is communicated in the signature.

## Doc Comment: Earned

```python
# Keep
def parse_duration(text: str) -> timedelta:
    """Parse a Go-style duration such as "1h30m".

    Raises ValueError on trailing garbage. A bare number is rejected; a unit is
    always required.
    """
```

In a published utility function whose callers do not read the implementation, this doc comment
provides information on how to call the function. Every sentence adds something not present in the
signature: the accepted format, the error, and a warning or rejection that callers might not expect.
