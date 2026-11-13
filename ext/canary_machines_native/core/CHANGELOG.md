# Changelog

## [0.4.0](https://github.com/seuros/canary_machines/compare/canary-machines-v0.3.0...canary-machines-v0.4.0) (2026-11-13)


### ⚠ BREAKING CHANGES

* see CONVENTIONS: one exhaustive Refusal per crate, Result from every mutator, honoured idempotent repeats only when provable, Duration time, Ruby mirrors the core. No compatibility aliases. Requires budget-machines >= 0.4. Ruby add(plan) replaces add_reference/add_paired; the Refused alias is gone.

### Features

* Duration observation instants, exact micro_nats, Plan, Model, Rate and Sprt as Ruby values ([6a030ec](https://github.com/seuros/canary_machines/commit/6a030ecb2819d7db4bf65b8637191eefa2a10a2d))

## [0.3.0](https://github.com/seuros/canary_machines/compare/canary-machines-v0.2.0...canary-machines-v0.3.0) (2026-11-13)


### Features

* rename Refused to Refusal and expose sliderule intervals on Plan ([f1c00b8](https://github.com/seuros/canary_machines/commit/f1c00b87c761726f5253e6dd5d4b2428f0c66b14))

## [0.2.0](https://github.com/seuros/canary_machines/compare/canary-machines-v0.1.0...canary-machines-v0.2.0) (2026-11-12)


### Features

* initial extraction ([c15b663](https://github.com/seuros/canary_machines/commit/c15b663ac5c81f21008773f568a96a27f935ae8b))
