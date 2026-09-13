# ADR 0012: Bounded text-or-list option observations

Status: accepted architecture declaration; validate and commit this architecture-only checkpoint before behavior. No new UCI recipe, device action, write authority or production dependency.

## Need and source semantics

The [reference rpcd UCI serializer](https://github.com/openwrt/rpcd/blob/e37ed9d814699098eb7e26c8b33c054840782dfb/uci.c) emits a UCI string as JSON text and a UCI list as an array of text. A scalar-only projection rejects legitimate lists, while accepting only arrays rejects legitimate strings. Splitting whitespace, joining lists, wrapping everything into an indistinguishable list or interpreting numbers would erase configuration representation. Generic raw JSON is not an acceptable solution.

Add one finite **nested** collection form to the existing pure core projector, not a new scalar union or alternate UCI decoder. Existing v11 closed config/type/get recipes, exact category Read requirements, root error/type guards, sessionless shared-delta semantics and denial of all custom UCI calls remain unchanged. No profile is added in this revision. Reads still do not establish committed-only or effective state.

## TextOption form and output

`Collection::TextOption { source, max_items, max_bytes }` is a terminal collection form with no record children, caller-chosen output names, selector or uniqueness flag. It is allowed only as a named collection inside a Record, never as a root TypedProjection::Collection and never with ExactOne selection. Generic finite Rust collection types still allow at most two collection levels. Its source is an explicit valid pointer with the same overlap rules as existing fields.

Admit only JSON strings or arrays whose elements are all JSON strings. A string produces `{"kind":"string","values":["original text"]}`; a list produces `{"kind":"list","values":["first","second"]}`. These two fixed output keys and discriminator values are core-owned, not metadata supplied by a client. Preserve string bytes, list order and duplicates. Do not split, join, trim, sort, deduplicate, normalize, expand globs, resolve paths/hosts or validate option semantics. A missing optional option stays absent, an empty string stays a one-element string representation, and an empty list stays a zero-element list representation. Null, mixed arrays, nested arrays, objects, booleans, numbers and NUL-containing text reject the whole observation.

Definition bounds: max_bytes 1..1024 UTF-8 bytes per text and max_items 1..128 values per option. The one string value consumes one item; a list consumes its actual length including duplicate/empty values. Each option counts as one collection schema node. Values consume the **existing shared scanned/emitted item budgets**, along with containing rows and all other nested collections; the total remains 256, nodes 8, fields 64, normalized output 65536 bytes and MCP result 262144 bytes. No separate per-field budget or silent truncation. Empty lists consume zero value items but their enclosing rows and fixed serialized structure remain bounded. Validate unselected rows too. Charge exact discriminator, key, array punctuation, escaped text and comma bytes before cloning/allocating output values.

The output wrapper is fixed structure, not a third collection level or arbitrary JSON container. Root arrays and earlier scalar/collection forms keep their existing envelope and behavior. No implicit conversion is added to ScalarKind::Text or ScalarArray.

## UCI consumers and layout

Core::uci permits only TextOption entries in a closed UCI read's nested collection declarations; ObjectArray, ObjectEntries, ScalarArray and RowArray remain forbidden there. The ordinary projection validator still enforces bounds and field/source collisions before I/O. Features continues to own exact non-secret scalar/option field lists and versioned response IDs in the existing category modules. Shared definition helpers only assemble declarations; no UCI execution, host API or permission decisions move into features.

First consumers extend network interface configuration with explicitly selected address/DNS/legacy device-list options and dnsmasq configuration with explicitly selected resolver/domain/interface/path options. Exact field names, limits, disclosure and response-ID changes must be documented and fixture-tested before feature behavior. Mount options, credentials, wireless BSS/SSID/keys and arbitrary configuration remain excluded. No additional probe or recipe is necessary. Old consumer evidence stays tied to its prior response version; a v11 successful scalar response does not validate a new optional list.

Runtime still executes PreparedInvocation through the same dispatcher and applies fallible projection before completion audit. Device-codec still owns strict action JSON decoding. MCP receives only normalized values, emits the same two bounded copies and never handles source configuration. Twelve crate owners and all dependency constraints remain unchanged.

## Harness and acceptance

Version 12 uses projection profile `typed_collections_v4` and includes TextOption among finite node forms. The declaration fixes nested-only admission, kind/values output, shared scan/emission/byte budgets and the 128-value cap. UCI contract adds only the exact `text_option_only` nested option declaration. Version 11 and earlier reject these new declarations/forms, even when all ordinary owner/probe settings are otherwise valid.

Architecture regressions must reject omitted/weakened/unknown text-option metadata, scalar/root/coercing/recursive alternatives, changed budgets, arbitrary UCI nested collections, older-version admission and missing required suites. Keep the existing native core/feature/runtime projection and MCP read/bounded-result suites mandatory; no declaration scaffold is counted as functional acceptance.

Implementation tests must distinguish absent/empty string/empty list/single-element list/multi-element list, preserve duplicates and Unicode exactly, reject malformed late values and root/selection misuse, and exercise individual/shared/depth/node/escaped-byte boundaries before output allocation. Exact UCI actions, old scalar behavior, category/direct-call denial, custom bypass denial, same-target capability checks, failed projection audit and both MCP copies remain covered. Synthetic, emulated and physical-device evidence remain separate. Full management, writes, recovery and platform-specific protection gaps are unaffected.
