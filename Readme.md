## Rust RTL-SDR waterfall demo

Just a little demo rendering a spectrogram of a signal captured by an RTL-SDR in
Rust using SDL2.

### Goals

* Basic interfacing with the RTL-SDR
* Have a FFT with [normalization to dBFS][fft_normalization]

### Font assets

- [`Andika`][andika]

  [andika]: https://software.sil.org/andika/
  [fft_normalization]: ./docs/fft_normalization.md
