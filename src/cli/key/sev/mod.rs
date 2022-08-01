// SPDX-License-Identifier: Apache-2.0

mod digest;

use clap::Subcommand;

/// SEV-specific functionality
#[derive(Subcommand, Debug)]
pub enum Subcommands {
    Digest(digest::Options),
}

impl Subcommands {
    pub fn dispatch(self) -> anyhow::Result<()> {
        match self {
            Self::Digest(cmd) => cmd.execute(),
        }
    }
}
