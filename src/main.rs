// SPDX-License-Identifier: Apache-2.0

//! This crate provides the `enarx` executable, which is a tool for running
//! code inside an Enarx Keep - that is a hardware isolated environment using
//! technologies such as Intel SGX or AMD SEV.
//!
//! # Building
//!
//! Please see **BUILD.md** for instructions.
//!
//! # Run Tests
//!
//!     $ cargo test
//!
//! # Build and run a WebAssembly module
//!
//!     $ cargo init --bin hello-world
//!     $ cd hello-world
//!     $ echo 'fn main() { println!("Hello, Enarx!"); }' > src/main.rs
//!     $ cargo build --release --target=wasm32-wasi
//!     $ enarx run target/wasm32-wasi/release/hello-world.wasm
//!     Hello, Enarx!
//!
//! # Select a Different Backend
//!
//! `enarx` will probe the machine it is running on in an attempt to deduce an
//! appropriate deployment backend. To see what backends are supported on your
//! system, run:
//!
//!     $ enarx info
//!
//! You can manually select a backend with the `--backend` option, or by
//! setting the `ENARX_BACKEND` environment variable:
//!
//!     $ enarx run --backend=sgx test.wasm
//!     $ ENARX_BACKEND=sgx enarx run test.wasm
//!
//! Note that some backends are conditionally compiled. They can all
//! be compiled in like so:
//!
//!     $ cargo build --all-features
//!
//! Or specific backends can be compiled in:
//!
//!     $ cargo build --features=backend-sgx,backend-kvm

#![deny(clippy::all)]
#![deny(missing_docs)]
#![feature(asm)]

mod backend;
mod cli;
mod protobuf;
mod workldr;

use backend::{Backend, Command};

use std::convert::TryInto;
use std::fs::File;
use std::os::unix::io::AsRawFd;

use anyhow::Result;
use log::info;
use structopt::StructOpt;

// This defines the toplevel `enarx` CLI
#[derive(StructOpt, Debug)]
#[structopt(
    setting = structopt::clap::AppSettings::DeriveDisplayOrder,
)]
struct Options {
    /// Logging options
    #[structopt(flatten)]
    log: cli::LogOptions,

    /// Subcommands (with their own options)
    #[structopt(flatten)]
    cmd: cli::Command,
}

#[allow(clippy::unnecessary_wraps)]
fn main() -> Result<()> {
    let opts = Options::from_args();
    opts.log.init_logger();

    info!("logging initialized!");
    info!("CLI opts: {:?}", &opts);

    match opts.cmd {
        cli::Command::Info(info) => info.display(),
        cli::Command::Exec(exec) => {
            let backend = exec.backend.pick()?;
            let binary = mmarinus::Kind::Private.load::<mmarinus::perms::Read, _>(&exec.code)?;
            keep_exec(backend, backend.shim(), binary)
        }
        cli::Command::Run(run) => {
            let modfile = File::open(run.module)?;
            let open_fd = modfile.as_raw_fd();
            // TODO: we should pass the fd to workldr, but we don't actually
            // have a way to pass arguments to workldr yet, so for now..
            assert!(open_fd == 3, "module got unexpected fd {}", open_fd);
            let backend = run.backend.pick()?;
            let workldr = run.workldr.pick()?;
            keep_exec(backend, backend.shim(), workldr.exec())
        }
    }
}

fn keep_exec(backend: &dyn Backend, shim: impl AsRef<[u8]>, exec: impl AsRef<[u8]>) -> Result<()> {
    let keep = backend.keep(shim.as_ref(), exec.as_ref())?;
    let mut thread = keep.clone().spawn()?.unwrap();
    loop {
        match thread.enter()? {
            Command::SysCall(block) => unsafe {
                block.msg.rep = block.msg.req.syscall();
            },

            Command::CpuId(block) => unsafe {
                let cpuid = core::arch::x86_64::__cpuid_count(
                    block.msg.req.arg[0].try_into().unwrap(),
                    block.msg.req.arg[1].try_into().unwrap(),
                );

                block.msg.req.arg[0] = cpuid.eax.into();
                block.msg.req.arg[1] = cpuid.ebx.into();
                block.msg.req.arg[2] = cpuid.ecx.into();
                block.msg.req.arg[3] = cpuid.edx.into();
            },

            Command::Continue => (),
        }
    }
}
