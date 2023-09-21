pub mod camera;
pub mod pipelines;
pub mod setup;
pub mod storage;
pub mod streaming;
pub mod textures;
pub mod vertex;

#[macro_export]
macro_rules! ensure {
    ($cond:expr, $err:expr) => {
        if !$cond {
            return Err($err.into());
        }
    }
}
