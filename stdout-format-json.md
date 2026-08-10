# Messages output when using `--stdout-format json`

Commands supporting `--stdout-format json` write newline-delimited JSON ([NDJSON](https://github.com/ndjson/ndjson-spec)) to stdout: one object per line, each with a `type` key identifying the message kind. Progress bars, logs, and hints go to stderr only, so stdout is parseable line by line.

Supported by: `sample-encode`, `crf-search`.

Notes:
* Later versions may add keys and message kinds; consumers should ignore unknown ones.
* Failures print an `Error: ...` line to stderr and exit non-zero in JSON mode too. A failed CRF search is also reported as JSON; see [`crf-search-error`](#crf-search-error).

## `sample-encode-done`

Emitted after all samples of a sample encode run are encoded and scored. `sample-encode` emits one at the end of the run; `crf-search` emits one per CRF attempt.

Field | Description | Type/Units
---|---|---
`type` | `"sample-encode-done"` | string
`crf` | Encoder CRF used | float
`from_cache` | All sample results were read from the cache | bool
`predicted_encode_percent` | Predicted output encode size percentage vs input | float
`predicted_encode_seconds` | Predicted output encode time in seconds | float
`predicted_encode_size` | Predicted output encode size in bytes | uint
`vmaf` | Mean sample VMAF score (present when requested) | float
`xpsnr` | Mean sample XPSNR score (present when requested) | float

### Example

```json
{"crf":29.25,"from_cache":false,"predicted_encode_percent":14.819840772420237,"predicted_encode_seconds":12.0,"predicted_encode_size":73692619,"type":"sample-encode-done","vmaf":95.37376403808594}
```

In versions through v0.11.4, `sample-encode` emitted this object without the `type`, `crf`, and `from_cache` keys.

## `crf-search-done`

Emitted when the search ends successfully, like the final human result line. The best CRF may have been decided by an earlier attempt, so this message can repeat a non-adjacent `sample-encode-done`.

It has the same fields as `sample-encode-done`, with `type` set to `"crf-search-done"`.

### Example

```json
{"crf":29.75,"from_cache":false,"predicted_encode_percent":13.785497397240093,"predicted_encode_seconds":13.0,"predicted_encode_size":68549279,"type":"crf-search-done","vmaf":95.16242980957031}
```

## `crf-search-error`

Emitted when the search fails to find a CRF satisfying the minimum score and maximum encoded percent. It follows the failing attempt's `sample-encode-done`. The process also writes `Error: ...` to stderr and exits non-zero.

Field | Description | Type/Units
---|---|---
`type` | `"crf-search-error"` | string
`message` | Failure description | string

### Example

```json
{"message":"Failed to find a suitable crf (last crf 18)","type":"crf-search-error"}
```

## Stream guarantees

`sample-encode` emits one `sample-encode-done`.

`crf-search` emits one `sample-encode-done` per attempted CRF and then:

* `crf-search-done` with exit code 0; or
* `crf-search-error` with a non-zero exit code when no suitable CRF exists.

Other errors, such as invalid input, end the stream without a final JSON message and are reported on stderr with a non-zero exit code.
