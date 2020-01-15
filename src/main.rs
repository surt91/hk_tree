use std::fs::File;
use std::io::prelude::*;

use structopt::StructOpt;

use hk::HegselmannKrause;

/// Simulate a  Hegselmann Krause model
#[derive(StructOpt, Debug)]
struct Opt {
    #[structopt(short, long)]
    /// number of interacting agents
    num_agents: u32,

    #[structopt(short = "l", long, default_value = "0.0")]
    /// minimum confidence of agents (uniformly distributed)
    min_confidence: f64,

    #[structopt(short = "u", long, default_value = "1.0")]
    /// maximum confidence of agents (uniformly distributed)
    max_confidence: f64,

    #[structopt(short, long, default_value = "1")]
    /// seed to use for the simulation
    seed: u64,

    #[structopt(long, default_value = "1")]
    /// number of times to repeat the simulation
    samples: u32,

    #[structopt(short, long, default_value = "out", parse(from_os_str))]
    /// name of the output data file
    outname: std::path::PathBuf,
}

fn main() -> std::io::Result<()> {
    let args = Opt::from_args();

    let mut hk = HegselmannKrause::new(
        args.num_agents,
        args.min_confidence as f32,
        args.max_confidence as f32,
        args.seed
    );

    let mut output = File::create(&args.outname)?;

    for _ in 0..args.samples {
        hk.reset();

        let mut ctr = 0;
        loop {
            ctr += 1;

            hk.sweep();

            // test if we are converged
            if hk.accumulated_change < 1e-4 {
                write!(output, "# sweeps: {}\n", ctr)?;
                break;
            }
            hk.accumulated_change = 0.;
        }
        hk.write_cluster_sizes(&mut output)?;
    }

    Ok(())
}
