use std::collections::BTreeMap;
use std::ops::Bound::Included;
use std::fmt;
use std::fs::File;
use std::io::prelude::*;

use rand::{Rng, SeedableRng};
use rand_pcg::Pcg64;
use itertools::Itertools;

use ordered_float::OrderedFloat;

/// numerical tolerance
const EPS: f32 = 1e-5;

#[derive(Clone, Debug)]
pub struct HKAgent {
    pub opinion: f32,
    pub confidence: f32,
}

impl HKAgent {
    fn new(opinion: f32, confidence: f32) -> HKAgent {
        HKAgent {
            opinion,
            confidence,
        }
    }
}

impl PartialEq for HKAgent {
    fn eq(&self, other: &HKAgent) -> bool {
        (self.opinion - other.opinion).abs() < EPS
            && (self.confidence - other.confidence).abs() < EPS
    }
}

pub struct HegselmannKrause {
    pub num_agents: u32,
    pub agents: Vec<HKAgent>,
    pub time: usize,
    min_confidence: f32,
    max_confidence: f32,

    pub opinion_set: BTreeMap<OrderedFloat<f32>, u32>,
    pub accumulated_change: f32,

    // we need many, good (but not crypto) random numbers
    // we will use here the pcg generator
    rng: Pcg64,
}

impl PartialEq for HegselmannKrause {
    fn eq(&self, other: &HegselmannKrause) -> bool {
        self.agents == other.agents
    }
}

impl fmt::Debug for HegselmannKrause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HK {{ N: {}, agents: {:?} }}", self.num_agents, self.agents)
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

        // datastructure for `step_tree`
        let opinion_set = BTreeMap::new();

        let mut hk = HegselmannKrause {
            num_agents: n,
            agents,
            time: 0,
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
        /// helper function to scale
        fn scale(x: f32, low: f32, high: f32) -> f32 {
            x*(high-low)+low
        }

        // initialize the vector of agents with uniformly distributed opinions and confidences
        self.agents = (0..self.num_agents).map(|_| HKAgent::new(
            self.rng.gen(),
            scale(self.rng.gen(), self.min_confidence, self.max_confidence),
        )).collect();

        // initialize the tree of opinions with the initial conditions of the agents
        // note that `OrderedFloat` is a technicality to allow using floats as keys in
        // the tree (rooted in the problem that IEEE floats do not have a total order)
        self.opinion_set.clear();
        for i in self.agents.iter() {
            *self.opinion_set.entry(OrderedFloat(i.opinion)).or_insert(0) += 1;
        }
        // assert that every agent has a corrsponding opinio in the tree
        assert!(self.opinion_set.iter().map(|(_, v)| v).sum::<u32>() == self.num_agents);

        self.time = 0;
    }

    /// update the internal datastructure in case, any opinion was updated
    fn update_entry(&mut self, old_opinion: f32, new_opinion: f32) {
        // often, nothing changes -> optimize for this converged case
        if old_opinion == new_opinion {
            return
        }

        // if something changes, we have to update the tree
        // decrease the counter of the old opinion and remove it, if the counter hits 0
        *self.opinion_set.entry(OrderedFloat(old_opinion))
            .or_insert_with(|| panic!("Removed opinion was not in the tree!")) -= 1;
        if self.opinion_set[&OrderedFloat(old_opinion)] == 0 {
            self.opinion_set.remove(&OrderedFloat(old_opinion));
        }
        // increase the counter of the new opinion or insert a new node for it
        *self.opinion_set.entry(OrderedFloat(new_opinion)).or_insert(0) += 1;
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

            let new_opinion = sum / count as f32;
            new_opinion
        }).collect()
    }

    // perform a sweep (update every agent) with the naive method
    pub fn sweep_naive(&mut self) {
        let new_opinions = self.sync_new_opinions_naive();
        self.accumulated_change = 0.;

        for i in 0..self.num_agents as usize {
            self.accumulated_change += (self.agents[i].opinion - new_opinions[i]).abs();

            self.agents[i].opinion = new_opinions[i];
        }
    }

    /// calculate all new opinions using the improved method using the tree
    fn sync_new_opinions_tree(&self) -> Vec<f32> {
        self.agents.clone().iter().map(|i| {
            let (sum, count) = self.opinion_set
                .range((Included(&OrderedFloat(i.opinion-i.confidence)), Included(&OrderedFloat(i.opinion+i.confidence))))
                .map(|(j, ctr)| (j.into_inner(), ctr))
                .fold((0., 0), |(sum, count), (j, ctr)| (sum + *ctr as f32 * j, count + ctr));

            let new_opinion = sum / count as f32;
            new_opinion
        }).collect()
    }

    // perform a sweep (update every agent) with the tree-based method
    pub fn sweep_tree(&mut self) {
        let new_opinions = self.sync_new_opinions_tree();
        self.accumulated_change = 0.;

        for i in 0..self.num_agents as usize {
            let old_opinion = self.agents[i].opinion;
            self.update_entry(old_opinion, new_opinions[i]);

            self.accumulated_change += (old_opinion - new_opinions[i]).abs();

            self.agents[i].opinion = new_opinions[i];
        }
    }

    pub fn sweep(&mut self) {
        // self.sweep_naive();
        self.sweep_tree();
        self.time += 1;
    }

    /// A cluster are agents whose distance is less than EPS
    fn list_clusters(&self) -> Vec<Vec<HKAgent>> {
        let mut clusters: Vec<Vec<HKAgent>> = Vec::new();
        'agent: for i in &self.agents {
            for c in &mut clusters {
                if (i.opinion - &c[0].opinion).abs() < EPS {
                    c.push(i.clone());
                    continue 'agent;
                }
            }
            clusters.push(vec![i.clone(); 1])
        }
        clusters
    }

    pub fn cluster_sizes(&self) -> Vec<usize> {
        let clusters = self.list_clusters();
        clusters.iter()
            .map(|c| c.len() as usize)
            .collect()
    }

    pub fn write_cluster_sizes(&self, file: &mut File) -> std::io::Result<()> {
        let clusters = self.list_clusters();

        let string_list = clusters.iter()
            .map(|c| c[0].opinion)
            .join(" ");
        write!(file, "# {}\n", string_list)?;

        let string_list = clusters.iter()
            .map(|c| c.len().to_string())
            .join(" ");
        write!(file, "{}\n", string_list)?;
        Ok(())
    }
}
