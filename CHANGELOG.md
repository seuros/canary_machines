# Changelog

## [0.7.1](https://github.com/seuros/canary_machines/compare/canary_machines/v0.7.0...canary_machines/v0.7.1) (2026-11-16)


### Bug Fixes

* fix wrapped classes at load so building a value runs no Ruby ([a5f164c](https://github.com/seuros/canary_machines/commit/a5f164c47525c8de405b5fcbc67ade83bb53375f))

## [0.7.0](https://github.com/seuros/canary_machines/compare/canary_machines/v0.6.0...canary_machines/v0.7.0) (2026-11-16)


### ⚠ BREAKING CHANGES

* Interval reads through lo()/hi(), Pairs#total is native

### Features

* Interval reads through lo()/hi(), Pairs#total is native ([4616d48](https://github.com/seuros/canary_machines/commit/4616d4808519f946bc3a9f31403249ae9bb6e9a4))

## [0.6.0](https://github.com/seuros/canary_machines/compare/canary_machines/v0.5.1...canary_machines/v0.6.0) (2026-11-14)


### ⚠ BREAKING CHANGES

* read sibling counts through own_or_sibling and models through which_fields

### Features

* read sibling counts through own_or_sibling and models through which_fields ([d1abfe5](https://github.com/seuros/canary_machines/commit/d1abfe5e281d6861fbf11b61f90010652f07b7d4))

## [0.5.1](https://github.com/seuros/canary_machines/compare/canary_machines/v0.5.0...canary_machines/v0.5.1) (2026-11-14)


### Bug Fixes

* **deps:** require matryona-machines &gt;= 0.3 with which, optional_fields and a States table ([a280ca4](https://github.com/seuros/canary_machines/commit/a280ca4c91dec19861d28cc4fedea887d0c06ef1))

## [0.5.0](https://github.com/seuros/canary_machines/compare/canary_machines/v0.4.0...canary_machines/v0.5.0) (2026-11-14)


### ⚠ BREAKING CHANGES

* build the binding on matryona-machines 0.2; check frozen before converting arguments; compiler-checked symbol tables; read every value from the struct

### Features

* build the binding on matryona-machines 0.2; check frozen before converting arguments; compiler-checked symbol tables; read every value from the struct ([9c906a2](https://github.com/seuros/canary_machines/commit/9c906a2dfa31aae0b5103e149e839568be649758))

## [0.4.0](https://github.com/seuros/canary_machines/compare/canary_machines/v0.3.0...canary_machines/v0.4.0) (2026-11-14)


### ⚠ BREAKING CHANGES

* build the binding on matryona-machines; drop the rb-sys feature; name every argument in conversion errors

### Features

* build the binding on matryona-machines; drop the rb-sys feature; name every argument in conversion errors ([76c96d0](https://github.com/seuros/canary_machines/commit/76c96d0c8d448372fbfa6375a5b358c6ecdbe50f))

## [0.3.0](https://github.com/seuros/canary_machines/compare/canary_machines/v0.2.0...canary_machines/v0.3.0) (2026-11-14)


### ⚠ BREAKING CHANGES

* validate and seal every value, private core value fields, exact-class boundary, clone-then-keep observe

### Features

* validate and seal every value, private core value fields, exact-class boundary, clone-then-keep observe ([501009e](https://github.com/seuros/canary_machines/commit/501009e8521714c7e442b8cb8b101e2fc9a4b292))

## [0.2.0](https://github.com/seuros/canary_machines/compare/canary_machines/v0.1.2...canary_machines/v0.2.0) (2026-11-13)


### ⚠ BREAKING CHANGES

* see CONVENTIONS: one exhaustive Refusal per crate, Result from every mutator, honoured idempotent repeats only when provable, Duration time, Ruby mirrors the core. No compatibility aliases. Requires budget-machines >= 0.4. Ruby add(plan) replaces add_reference/add_paired; the Refused alias is gone.

### Features

* Duration observation instants, exact micro_nats, Plan, Model, Rate and Sprt as Ruby values ([6a030ec](https://github.com/seuros/canary_machines/commit/6a030ecb2819d7db4bf65b8637191eefa2a10a2d))

## [0.1.2](https://github.com/seuros/canary_machines/compare/canary_machines/v0.1.1...canary_machines/v0.1.2) (2026-11-13)


### Features

* rename Refused to Refusal and expose sliderule intervals on Plan ([f1c00b8](https://github.com/seuros/canary_machines/commit/f1c00b87c761726f5253e6dd5d4b2428f0c66b14))

## [0.1.1](https://github.com/seuros/canary_machines/compare/canary_machines-v0.1.0...canary_machines/v0.1.1) (2026-11-12)


### Features

* initial extraction ([c15b663](https://github.com/seuros/canary_machines/commit/c15b663ac5c81f21008773f568a96a27f935ae8b))
