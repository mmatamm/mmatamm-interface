pub mod c_api;

use std::ffi::OsStr;

use anyhow::ensure;
#[cfg(unix)]
use libloading::os::unix as libloading_imp;
#[cfg(windows)]
use libloading::os::windows as libloading_imp;

use crate::market::Market;
use c_api::CApiMarket;

pub struct ExternalAlgorithm {
    library: libloading::Library,
    run_function: libloading_imp::Symbol<unsafe extern "C" fn(&mut CApiMarket, *const f32) -> ()>,
    no_parameters: usize,
}

impl ExternalAlgorithm {
    pub unsafe fn new<P: AsRef<OsStr>>(filename: P) -> Result<Self, libloading::Error> {
        let library = libloading::Library::new(filename)?;

        let run: libloading::Symbol<unsafe extern "C" fn(&mut CApiMarket, *const f32) -> ()> =
            library.get(b"run\0")?;
        let run_raw = run.into_raw();
        let no_parameters: libloading::Symbol<*const usize> = library.get(b"no_parameters\0")?;
        let no_parameters_raw = *no_parameters.into_raw();

        // TODO Support wake_ups

        Ok(Self {
            library,
            run_function: run_raw,
            no_parameters: *no_parameters_raw,
        })
    }

    pub fn run<M>(&mut self, market: &mut M, parameters: &[f32]) -> anyhow::Result<()>
    where
        M: Market<Error = anyhow::Error>,
    {
        ensure!(
            parameters.len() == self.no_parameters,
            "Received {} parameters while the algorithm needs {}",
            parameters.len(),
            self.no_parameters
        );

        let mut c_market = CApiMarket::new(market);

        unsafe { (self.run_function)(&mut c_market, parameters.as_ptr()) }

        c_market.result()
    }
}
