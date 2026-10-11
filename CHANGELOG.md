# Changelog

## [0.1.5](https://github.com/matt-riley/crap/compare/v0.1.4...v0.1.5) (2026-10-11)


### Bug Fixes

* **release:** create the tag along with the draft release ([92cc54b](https://github.com/matt-riley/crap/commit/92cc54bb82f6288fcdb1b5777211fc9816395b0c))

## [0.1.4](https://github.com/matt-riley/crap/compare/v0.1.3...v0.1.4) (2026-10-11)


### Features

* **baseline:** gate on new or worse offenders ([a288b64](https://github.com/matt-riley/crap/commit/a288b64edec8394f65c9ac2ee4c54e9967615f27))
* **diff:** score only the functions a branch touched ([7358a48](https://github.com/matt-riley/crap/commit/7358a4890a55d485833462a9505518b75be1fa24))
* **report:** emit github workflow annotations ([820aa27](https://github.com/matt-riley/crap/commit/820aa27bcc623d041aaa1db3ff04b74862a86bf5))
* token-aware CRAP scoring for TypeScript, Rust, Go and Lua ([b3a374d](https://github.com/matt-riley/crap/commit/b3a374d71b979727133cdf0b30fffd4d8c54b33d))


### Bug Fixes

* **ci:** install rustfmt and clippy in the mise toolchain ([1b6073b](https://github.com/matt-riley/crap/commit/1b6073b3573e030a26988325794cd4647a8b8583))
* **coverage:** discover reports in the scanned tree ([f174b3c](https://github.com/matt-riley/crap/commit/f174b3c379b978be5e67c13cb610e246c4a9ebc2))
* **coverage:** merge reports that name the same file ([526fc85](https://github.com/matt-riley/crap/commit/526fc85553b4636c1979ed4a769dc6f36d955a26))
* **diff:** match suffixed paths only at a path boundary ([eb97398](https://github.com/matt-riley/crap/commit/eb97398ddb9409b2f360a61d2011bd2b46f3beaa))
* **diff:** quote one line of git's output, not its whole usage text ([da90144](https://github.com/matt-riley/crap/commit/da90144d684d47dc35f0b13990fd73aaae7112e5))
* **diff:** stop git from reading a rev as an option ([57edaf0](https://github.com/matt-riley/crap/commit/57edaf06dc6cac794cb9e08078487de9cda70d36))
* **lang:** count switch and select defaults ([fba4b14](https://github.com/matt-riley/crap/commit/fba4b140c1f9beb63fff1838206757af64d98ad5))
* **main:** report files that could not be read ([dafe83e](https://github.com/matt-riley/crap/commit/dafe83eda05250a9a133e53b164b56091bfd8c15))
* **release:** publish the tap from a run the tag actually triggered ([bbeb19f](https://github.com/matt-riley/crap/commit/bbeb19fb4632b02800bee1201cec895939c842a7))
* **report:** honour --top in text output ([5c30fab](https://github.com/matt-riley/crap/commit/5c30fab6cb02546eb572f9d8d957f7b14ee3b83d))
* **score:** stop treating cfg(not(test)) as test code ([f58abdf](https://github.com/matt-riley/crap/commit/f58abdf6a8489134dd987fe5f2728718bdd84bba))

## [0.1.3](https://github.com/matt-riley/crap/compare/v0.1.2...v0.1.3) (2026-10-11)


### Features

* **baseline:** gate on new or worse offenders ([a288b64](https://github.com/matt-riley/crap/commit/a288b64edec8394f65c9ac2ee4c54e9967615f27))
* **report:** emit github workflow annotations ([820aa27](https://github.com/matt-riley/crap/commit/820aa27bcc623d041aaa1db3ff04b74862a86bf5))


### Bug Fixes

* **coverage:** discover reports in the scanned tree ([f174b3c](https://github.com/matt-riley/crap/commit/f174b3c379b978be5e67c13cb610e246c4a9ebc2))
* **coverage:** merge reports that name the same file ([526fc85](https://github.com/matt-riley/crap/commit/526fc85553b4636c1979ed4a769dc6f36d955a26))
* **diff:** match suffixed paths only at a path boundary ([eb97398](https://github.com/matt-riley/crap/commit/eb97398ddb9409b2f360a61d2011bd2b46f3beaa))
* **diff:** stop git from reading a rev as an option ([57edaf0](https://github.com/matt-riley/crap/commit/57edaf06dc6cac794cb9e08078487de9cda70d36))
* **lang:** count switch and select defaults ([fba4b14](https://github.com/matt-riley/crap/commit/fba4b140c1f9beb63fff1838206757af64d98ad5))
* **main:** report files that could not be read ([dafe83e](https://github.com/matt-riley/crap/commit/dafe83eda05250a9a133e53b164b56091bfd8c15))
* **report:** honour --top in text output ([5c30fab](https://github.com/matt-riley/crap/commit/5c30fab6cb02546eb572f9d8d957f7b14ee3b83d))
* **score:** stop treating cfg(not(test)) as test code ([f58abdf](https://github.com/matt-riley/crap/commit/f58abdf6a8489134dd987fe5f2728718bdd84bba))

## [0.1.2](https://github.com/matt-riley/crap/compare/v0.1.1...v0.1.2) (2026-10-09)


### Features

* **diff:** score only the functions a branch touched ([7358a48](https://github.com/matt-riley/crap/commit/7358a4890a55d485833462a9505518b75be1fa24))
* token-aware CRAP scoring for TypeScript, Rust, Go and Lua ([b3a374d](https://github.com/matt-riley/crap/commit/b3a374d71b979727133cdf0b30fffd4d8c54b33d))


### Bug Fixes

* **ci:** install rustfmt and clippy in the mise toolchain ([1b6073b](https://github.com/matt-riley/crap/commit/1b6073b3573e030a26988325794cd4647a8b8583))
* **diff:** quote one line of git's output, not its whole usage text ([da90144](https://github.com/matt-riley/crap/commit/da90144d684d47dc35f0b13990fd73aaae7112e5))
* **release:** publish the tap from a run the tag actually triggered ([bbeb19f](https://github.com/matt-riley/crap/commit/bbeb19fb4632b02800bee1201cec895939c842a7))

## [0.1.1](https://github.com/matt-riley/crap/compare/crap-cli-v0.1.0...crap-cli-v0.1.1) (2026-10-09)


### Features

* **diff:** score only the functions a branch touched ([7358a48](https://github.com/matt-riley/crap/commit/7358a4890a55d485833462a9505518b75be1fa24))
* token-aware CRAP scoring for TypeScript, Rust, Go and Lua ([b3a374d](https://github.com/matt-riley/crap/commit/b3a374d71b979727133cdf0b30fffd4d8c54b33d))


### Bug Fixes

* **ci:** install rustfmt and clippy in the mise toolchain ([1b6073b](https://github.com/matt-riley/crap/commit/1b6073b3573e030a26988325794cd4647a8b8583))
* **diff:** quote one line of git's output, not its whole usage text ([da90144](https://github.com/matt-riley/crap/commit/da90144d684d47dc35f0b13990fd73aaae7112e5))
