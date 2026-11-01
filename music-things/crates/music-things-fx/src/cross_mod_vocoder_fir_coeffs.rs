//! Source Warps 3×/36 and 4×/48 symmetric FIR half-kernels.
//!
//! Copyright 2015 Emilie Gillet. Author: Emilie Gillet.
//! Permission is hereby granted, free of charge, to any person obtaining a
//! copy of this software and associated documentation files (the "Software"),
//! to deal in the Software without restriction, including without limitation
//! the rights to use, copy, modify, merge, publish, distribute, sublicense,
//! and/or sell copies of the Software, and to permit persons to whom the
//! Software is furnished to do so, subject to the following conditions:
//! The above copyright notice and this permission notice shall be included
//! in all copies or substantial portions of the Software.
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
//! OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
//! MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
//! IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
//! DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
//! OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE
//! USE OR OTHER DEALINGS IN THE SOFTWARE.
//!
//! Individually extracted from MIT `warps/dsp/sample_rate_conversion_filters.h`
//! at eurorack 08460a69. These are FIR coefficients, not waveform assets.

#[allow(clippy::excessive_precision)]
pub(super) const UP4_HALF: [f32; 24] = [
    -6.014371929e-04,
    -1.116027480e-03,
    -1.547569918e-03,
    -1.288608084e-03,
    2.786886230e-04,
    3.529342828e-03,
    8.203156385e-03,
    1.308970614e-02,
    1.600199910e-02,
    1.419074690e-02,
    5.231038872e-03,
    -1.177915684e-02,
    -3.506738553e-02,
    -5.953252182e-02,
    -7.699933415e-02,
    -7.757902368e-02,
    -5.198496872e-02,
    5.703716839e-03,
    9.559598586e-02,
    2.106660616e-01,
    3.371310483e-01,
    4.566603688e-01,
    5.500087786e-01,
    6.012053946e-01,
];

#[allow(clippy::excessive_precision)]
pub(super) const DOWN4_HALF: [f32; 24] = [
    -1.503592982e-04,
    -2.790068700e-04,
    -3.868924795e-04,
    -3.221520211e-04,
    6.967215575e-05,
    8.823357070e-04,
    2.050789096e-03,
    3.272426536e-03,
    4.000499774e-03,
    3.547686724e-03,
    1.307759718e-03,
    -2.944789209e-03,
    -8.766846381e-03,
    -1.488313045e-02,
    -1.924983354e-02,
    -1.939475592e-02,
    -1.299624218e-02,
    1.425929210e-03,
    2.389899647e-02,
    5.266651541e-02,
    8.428276207e-02,
    1.141650922e-01,
    1.375021946e-01,
    1.503013486e-01,
];

#[allow(clippy::excessive_precision)]
pub(super) const UP3_HALF: [f32; 18] = [
    2.111177486e-04,
    9.399136027e-04,
    2.516356933e-03,
    4.847507152e-03,
    6.912087023e-03,
    6.524576194e-03,
    8.579855461e-04,
    -1.203466052e-02,
    -3.103696515e-02,
    -5.013495031e-02,
    -5.827142630e-02,
    -4.183809689e-02,
    1.038391226e-02,
    1.014554664e-01,
    2.222529437e-01,
    3.515426263e-01,
    4.610075226e-01,
    5.238640837e-01,
];

#[allow(clippy::excessive_precision)]
pub(super) const DOWN3_HALF: [f32; 18] = [
    7.037258286e-05,
    3.133045342e-04,
    8.387856444e-04,
    1.615835717e-03,
    2.304029008e-03,
    2.174858731e-03,
    2.859951820e-04,
    -4.011553507e-03,
    -1.034565505e-02,
    -1.671165010e-02,
    -1.942380877e-02,
    -1.394603230e-02,
    3.461304086e-03,
    3.381848881e-02,
    7.408431457e-02,
    1.171808754e-01,
    1.536691742e-01,
    1.746213612e-01,
];
