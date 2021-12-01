pub mod noise;
pub mod parabola;
pub mod signal;

pub use noise::*;
pub use parabola::*;
pub use signal::*;


pub fn lerp<const N: usize>(a: [f32; N], b: [f32; N], t: f32) -> [f32; N] {
    let mut res = [0.0; N];
    for k in 0..3 {
        res[k] = a[k] * (1.0 - t) + b[k] * t;
    }
    res
}

pub fn compute_mse(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len().min(b.len()) as f32;
    a.iter()
        .zip(b.iter())
        .fold(0.0, |acc, (y, y_est)| acc + (y - y_est).powf(2.0))
        / n
}
