# TACT Parser

Parser for various [TACT file formats][tact-ff], as they are used on the NGDP
CDN.

Some other community tooling describes these as ["CASC files"][casc]; but CASC
refers to the virtual filesystem used by locally-installed copies of games.

## Roadmap

The plan is to read enough of the the TACT files to allow [`ngdp-client`][] and
[`tact-client`][] to download and extract WoW data files from the NGDP CDN.

This is not yet integrated with the rest of `cascette`, and has been tested
using existing cached copies of CDN data from
[SimulationCraft's `casc_extract.py`][simc].

- [x] Read [WoW Root][wow-root], to find the file ID and MD5s of each variant
  of game data files (eg: `.db2`)

- [x] Read [CDN config][cdn-config], to get a list of data archives

- [x] Read [archive indexes][archive-index], to find where file fragments are
  within an archive

- [x] Read [encoding table][encoding], to find the BLTE file keys for a game
  data file MD5

- [x] Read [BLTE files][blte], to get file content from archives

- [ ] Read [patch files][patch]

[`ngdp-client`]: ../ngdp-client/
[`tact-client`]: ../tact-client/
[archive-index]: https://wowdev.wiki/TACT#Archive_Indexes_(.index)
[blte]: https://wowdev.wiki/BLTE
[casc]: https://wowdev.wiki/CASC
[cdn-config]: https://wowdev.wiki/TACT#CDN_Config
[encoding]: https://wowdev.wiki/TACT#Encoding_table
[patch]: https://wowdev.wiki/TACT#Patch
[simc]: https://github.com/simulationcraft/simc/blob/thewarwithin/casc_extract/
[tact-ff]: https://wowdev.wiki/TACT#File_types
[wow-root]: https://wowdev.wiki/TACT#Root
