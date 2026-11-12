surfaces: web, macos
agent: Claude
task: a weather screen that reads Open-Meteo with no data module (expo/exact#359)

### Rough

A screen that only shows JSON from one endpoint needed an `app.ts`: a grant, a `fetch`, a status check and a
mapping into the shape, and with it Hermes in the native build. exact2 now has sources declared in Contract
(LLP 1162.001), and this app is their first consumer: both of its reads are declared, so it has no `app.ts`.

The JSON's keys are the shape's field names (`temperature_2m`), since a declared answer is read field by field
and renaming is a `derive`'s. A search with no match leaves `results` out of the reply, so `Found` keeps it
`option<Place[]>` and the view reads `none` as an empty list.

### Lean in

`when query.trim().length >= 2` keeps the search from asking for one letter; `current(found)` tells "no places"
from "still asking". `failed(now)` and `refresh now` are the whole of the error and retry.

### Needed

- a read from an HTTP endpoint with no data module: provided (declared sources)
- an origin from `app.json`'s `env`: not provided; this app names Open-Meteo's origins literally

### Checkpoints

- web build: see the commit that adds the app
- macOS build without Hermes: see the commit that adds the app
