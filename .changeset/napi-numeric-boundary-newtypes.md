---
michi-node: minor
---

Every numeric argument crossing the NAPI boundary is now validated through a dedicated newtype (`JsCount`, `JsInt`, `JsRanged`/`JsDecimals`/`JsRetryCount`/`JsHttpStatus`, `JsFloat`, `JsUnitInterval`, `JsDelayMillis`) instead of ad hoc per-call-site coercion. Generated TypeScript signatures are unchanged (still plain `number`), but out-of-domain values that were previously silently coerced now throw:

- `truncate`'s `maxChars` and `JsToonOptions`/`JsAgentResponse`'s `totalCount`: a negative or fractional value was previously clamped to `0` (or, for `JsToonOptions.totalCount`, silently dropped); now rejected with a thrown error.
- `JsToonValue.decimalsVal`: a value outside `[0, 20]` was previously clamped into range; now rejected.
- `nextRetryDelay`'s `maxRetries`/`attempt`, `baseDelayMs`/`maxDelayMs`, `jitterFactor`/`jitterSeed`, and `retryAfterMs`: all seven parameters now go through the same validating newtypes and reject non-finite, negative, fractional (where integral), or overflow-inducing values with a consistent error message, instead of the previous ad hoc validation that only covered a subset of these parameters.
- `isRetryableStatus`'s `status`: a value outside `[100, 599]` was previously coerced to `0` via a fallible cast with a silent fallback; now rejected.
- `JsToonValue.intVal` widened from a 32-bit range to the full JS-safe-integer range (`±9007199254740991`), so large integers (for example millisecond timestamps) round-trip losslessly through `renderToon`/`renderKv` instead of being truncated to 32 bits.
