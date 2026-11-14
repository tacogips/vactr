//! Original RK4 integration of continuous Lorenz and Rössler equations.
//! Work is bounded to 32 substeps; finite guard resets pathological state.
use super::{bounded, Inp, Kx, NodeState, MAX_PORTS};

fn derivative(s: [f32; 3], chaos: f32, rossler: bool) -> [f32; 3] {
    let [x, y, z] = s;
    if rossler {
        [
            -y - z,
            x + (0.1 + 0.2 * chaos) * y,
            0.2 + z * (x - (4.0 + 4.0 * chaos)),
        ]
    } else {
        [
            10.0 * (y - x),
            x * ((20.0 + 16.0 * chaos) - z) - y,
            x * y - (8.0 / 3.0) * z,
        ]
    }
}
fn offset(s: [f32; 3], d: [f32; 3], dt: f32) -> [f32; 3] {
    [s[0] + dt * d[0], s[1] + dt * d[1], s[2] + dt * d[2]]
}
fn run(
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    out: &mut [f32],
    kx: &Kx<'_>,
    rossler: bool,
) {
    if st.u[0] == 0 {
        st.s[..3].copy_from_slice(if rossler {
            &[0.1, 0.0, 0.0]
        } else {
            &[0.1, 0.1, 0.1]
        });
        st.u[0] = 1;
    }
    let sr = kx.sr.max(100.0);
    for (i, y) in out.iter_mut().enumerate() {
        let rate = bounded(ins[0].at(i), 1.0, 0.01, 100.0);
        let chaos = bounded(ins[1].at(i), 0.5, 0.0, 1.0);
        let axis = bounded(ins[2].at(i), 0.0, 0.0, 2.0).round() as usize;
        let total = (40.0 * rate / sr).min(0.16);
        let steps = (total / 0.005).ceil().clamp(1.0, 32.0) as usize;
        let dt = total / steps as f32;
        let mut state = [st.s[0], st.s[1], st.s[2]];
        for _ in 0..steps {
            let a = derivative(state, chaos, rossler);
            let b = derivative(offset(state, a, dt * 0.5), chaos, rossler);
            let c = derivative(offset(state, b, dt * 0.5), chaos, rossler);
            let d = derivative(offset(state, c, dt), chaos, rossler);
            for j in 0..3 {
                state[j] += dt * (a[j] + 2.0 * b[j] + 2.0 * c[j] + d[j]) / 6.0;
            }
            if !state.iter().all(|v| v.is_finite() && v.abs() < 200.0) {
                state = [0.1, 0.1, 0.1];
            }
        }
        st.s[..3].copy_from_slice(&state);
        let center = if axis == 2 {
            if rossler {
                1.0
            } else {
                25.0
            }
        } else {
            0.0
        };
        let scale = if rossler { 12.0 } else { 25.0 };
        *y = ((state[axis] - center) / scale).tanh();
    }
}
/// Lorenz attractor, rate in simulation speed, chaos adjusts rho.
pub fn lorenz(ins: &[Inp<'_>; MAX_PORTS], st: &mut NodeState, out: &mut [f32], kx: &Kx<'_>) {
    run(ins, st, out, kx, false);
}
/// Rössler attractor, rate in simulation speed, chaos adjusts a and c.
pub fn rossler(ins: &[Inp<'_>; MAX_PORTS], st: &mut NodeState, out: &mut [f32], kx: &Kx<'_>) {
    run(ins, st, out, kx, true);
}
