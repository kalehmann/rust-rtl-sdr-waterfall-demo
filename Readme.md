## Rust RTL-SDR waterfall demo

Just a little demo rendering a spectrogram of a signal captured by an RTL-SDR in
Rust using SDL2.

### Goals

* Basic interfacing with the RTL-SDR
* Have a FFT with [normalization to dBFS][fft_normalization]

### Colors

The colors are loosely inspired by [this reddit post][color_palette].

### Font assets

- [`Andika`][andika]

  [andika]: https://software.sil.org/andika/
  [color_palette]: https://old.reddit.com/r/outrun/comments/zf7dfo/synthwave_color_palette_this_work_of_art_is_not/
  [fft_normalization]: ./docs/fft_normalization.md
