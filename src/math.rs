pub mod noise;
pub mod parabola;
pub mod signal;

pub use noise::*;
pub use parabola::*;
pub use signal::*;


pub type Vec<T,const N:usize> = [T;N]; 

pub fn lerp<const N: usize>(a: Vec<f32,N>, b: Vec<f32,N>, t: f32) -> Vec<f32,N> {
    let mut res = [0.0; N];
    for k in 0..N {
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
pub fn linear_step(x:f32,e0:f32, e1:f32)->f32{
    ((x-e0)/(e1-e0)).clamp(0.0,1.0)
}
