## FFT Normalization

One of goals of this demo application is to have the FFT output signal strengths
in dBFS similarly to how signal strength is displayed in [Gqrx][gqrx] or
[SDR++][sdrpp].
What do these two applications do?

### Gqrx

* First the method `rx_fft_c::get_fft_data` in `src/dsp/rx_fft.cpp` calls
  `rx_fft_c::apply_window` which copies data from the GNU radio buffer into the
  FFT input buffer.
* Then again in `rx_fft_c::get_fft_data` the FFT is executed and the results are
  afterwards mapped to magnitudes with `std::norm`

  **Note** that `std::norm` is not equivalent to `num::complex::Complex::norm`,
  but `num::complex::Complex::norm_sqr` in rust.
* Subsequently the method `MainWindow::iqFftTimeout` in
  `src/applications/gqrx/mainwindow.cpp` passes the mapped FFT results to
  `CPlotter::setNewFftData` in `src/qtgui/plotter.cpp`.
* That method introduces a variable `pwr_scale`, which (if dBFS are choosen as
  unit) equals `1.0 / (fft_size * fft_size)` and the multiplies each value in
  the FFT result which that variable.

  **Note** there is also a safety check, that the values are still larger than
  zero to avoid weird log10 results: `std::max(val * pwr_scale, 1e-20)`.

  At the end of the method, a redrawing of the UI is triggered.
* Finally in `CPlotter::draw` the fft data is converted using
  `10.0f * log10(val)`


  [gqrx]: https://github.com/gqrx-sdr/gqrx
  [sdrpp]: https://github.com/AlexandreRouma/SDRPlusPlus
