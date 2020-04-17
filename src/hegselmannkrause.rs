/// This file implements the Hegselmann-Krause bounded confidence model with heterogeneous
/// confidences and synchronous update. It implements two different algorithms to update
/// the agents:
/// `sweep_naive` uses the classical method of iterating over all agents to find those
///               within the confidence interval for the calculation of the next state
/// `sweep_tree`  uses the improved algorithm, based on a search tree (here a BTree), introduced
///               in the corresponding article

use std::collections::BTreeMap;
use std::ops::Bound::Included;
use std::fs::File;
use std::io::prelude::*;

use rand::{Rng, SeedableRng};
use rand_pcg::Pcg64;
use itertools::Itertools;

// note that `OrderedFloat` is a technicality to allow using floats as keys in
// the tree (rooted in the problem that IEEE floats do not have a total order, due to `nan`,
// which can therefore not be part of a search tree)
use ordered_float::OrderedFloat;

/// numerical tolerance
const EPS: f32 = 1e-5;

/// structure representing an agent
#[derive(Clone, Debug)]
struct HKAgent {
    /// current opinion of the agent
    opinion: f32,
    /// idiosyncratic confidence of the agent
    confidence: f32,
}

impl HKAgent {
    fn new(opinion: f32, confidence: f32) -> HKAgent {
        HKAgent {
            opinion,
            confidence,
        }
    }
}

/// used for testing purposes
impl PartialEq for HKAgent {
    fn eq(&self, other: &HKAgent) -> bool {
        (self.opinion - other.opinion).abs() < EPS
            && (self.confidence - other.confidence).abs() < EPS
    }
}

/// structure representing a realization of the HK model
pub struct HegselmannKrause {
    /// number of agents in the system
    num_agents: u32,
    /// vector of all agents constituting the system
    agents: Vec<HKAgent>,
    /// lower bound of the confidences of all agents
    min_confidence: f32,
    /// upper bound of the confidences of all agents
    max_confidence: f32,

    /// the tree structure used to efficiently update the system
    opinion_set: BTreeMap<OrderedFloat<f32>, u32>,
    /// total change of agents opinion during the last sweep
    pub accumulated_change: f32,

    /// we need many, good (but not crypto) random numbers
    /// we will use here the pcg generator
    rng: Pcg64,
}

/// used for testing purposes
impl PartialEq for HegselmannKrause {
    fn eq(&self, other: &HegselmannKrause) -> bool {
        self.agents == other.agents
    }
}

impl HegselmannKrause {
    pub fn new(
        n: u32,
        min_confidence: f32,
        max_confidence: f32,
        seed: u64
    ) -> HegselmannKrause {
        let rng = Pcg64::seed_from_u64(seed);
        let agents: Vec<HKAgent> = Vec::new();

        let opinion_set = BTreeMap::new();

        let mut hk = HegselmannKrause {
            num_agents: n,
            agents,
            min_confidence,
            max_confidence,
            opinion_set,
            accumulated_change: 0.,
            rng,
        };

        hk.reset();
        hk
    }

    /// reset the state of an HegselmannKrause struct
    /// initialize the agents with random initial conditions
    /// and prepare all internal datastructures
    /// afterwards the object will be ready for a fresh simulation
    pub fn reset(&mut self) {
        /// helper function to scale a uniform[0,1] random number to a uniform[low, high]
        fn scale(x: f32, low: f32, high: f32) -> f32 {
            x*(high-low)+low
        }

        // initialize a vector of n agents with uniformly distributed opinions and confidences
        self.agents = (0..self.num_agents).map(|_| HKAgent::new(
            self.rng.gen(),
            scale(self.rng.gen(), self.min_confidence, self.max_confidence),
        )).collect();

        // initialize the tree of opinions with the initial conditions of the agents
        self.opinion_set.clear();
        for i in self.agents.iter() {
            *self.opinion_set.entry(i.opinion.into()).or_insert(0) += 1;
        }

        // assert that every agent has a corresponding opinion in the tree
        assert!(self.opinion_set.iter().map(|(_, v)| v).sum::<u32>() == self.num_agents);
    }

    /// calculate all new opinions using the naive method of iterating all agents
    fn sync_new_opinions_naive(&self) -> Vec<f32> {
        self.agents.iter().map(|i| {
            let mut sum = 0.;
            let mut count = 0;
            for j in self.agents.iter()
                    .filter(|j| (i.opinion - j.opinion).abs() < i.confidence) {
                sum += j.opinion;
                count += 1;
            }

            sum / count as f32
        }).collect()
    }

    // perform a sweep (update every agent) with the naive method
    pub fn sweep_naive(&mut self) {
        let new_opinions = self.sync_new_opinions_naive();
        self.accumulated_change = 0.;

        for (i, &new_opinion) in new_opinions.iter().enumerate() {
            self.accumulated_change += (self.agents[i].opinion - new_opinion).abs();

            self.agents[i].opinion = new_opinion;
        }
    }

    // we use float comparision to test if an entry did change during an iteration for performance
    // false negatives do not lead to wrong results
    #[allow(clippy::float_cmp)]
    /// update the internal datastructure in case, any opinion was updated
    fn update_entry(&mut self, old_opinion: f32, new_opinion: f32) {
        // often, nothing changes -> optimize for this converged case
        if old_opinion == new_opinion {
            return
        }

        // if something changes, we have to update the tree
        // decrease the counter of the old opinion and remove it, if the counter hits 0
        *self.opinion_set.entry(old_opinion.into())
            .or_insert_with(|| panic!("Removed opinion was not in the tree!")) -= 1;
        if self.opinion_set[&old_opinion.into()] == 0 {
            self.opinion_set.remove(&old_opinion.into());
        }
        // increase the counter of the new opinion or insert a new node for it
        *self.opinion_set.entry(new_opinion.into()).or_insert(0) += 1;
    }

    /// calculate all new opinions using the improved method using the tree
    fn sync_new_opinions_tree(&self) -> Vec<f32> {
        self.agents.iter().map(|i| {
            let (sum, count) = self.opinion_set
                // this method traverses the tree starting from i.opinion-i.confidence
                // up to i.opinion+i.confidence
                .range(
                    (
                        Included(&OrderedFloat(i.opinion-i.confidence)),
                        Included(&OrderedFloat(i.opinion+i.confidence))
                    )
                )
                // into_inner converts an `OrderedFloat` into a f32
                .map(|(x, ctr)| (x.into_inner(), ctr))
                .fold((0., 0), |(sum, count), (x, ctr)| (sum + *ctr as f32 * x, count + ctr));

            sum / count as f32
        }).collect()
    }

    // perform a sweep (update every agent) with the tree-based method
    pub fn sweep_tree(&mut self) {
        let new_opinions = self.sync_new_opinions_tree();
        self.accumulated_change = 0.;

        for (i, &new_opinion) in new_opinions.iter().enumerate() {
            let old_opinion = self.agents[i].opinion;
            self.update_entry(old_opinion, new_opinion);

            self.accumulated_change += (old_opinion - new_opinion).abs();

            self.agents[i].opinion = new_opinion;
        }
    }

    pub fn sweep(&mut self) {
        // self.sweep_naive();
        self.sweep_tree();
    }

    /// A cluster are agents whose distance is less than EPS
    fn list_clusters(&self) -> Vec<Vec<HKAgent>> {
        let mut clusters: Vec<Vec<HKAgent>> = Vec::new();
        'agent: for i in &self.agents {
            for c in &mut clusters {
                if (i.opinion - c[0].opinion).abs() < EPS {
                    c.push(i.clone());
                    continue 'agent;
                }
            }
            clusters.push(vec![i.clone(); 1])
        }
        clusters
    }

    pub fn cluster_sizes(&self) -> Vec<usize> {
        self.list_clusters()
            .iter()
            .map(|c| c.len() as usize)
            .collect()
    }

    pub fn write_cluster_sizes(&self, file: &mut File) -> std::io::Result<()> {
        let clusters = self.list_clusters();

        // write positions of the clusters
        let string_list = clusters.iter()
            .map(|c| c[0].opinion)
            .join(" ");
        writeln!(file, "# {}", string_list)?;

        // write sizes of the clusters
        let string_list = clusters.iter()
            .map(|c| c.len().to_string())
            .join(" ");
        writeln!(file, "{}", string_list)?;
        Ok(())
    }
}
