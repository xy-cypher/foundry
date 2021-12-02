use ethers::{
    abi::{Abi, FunctionExt, RawLog},
    types::{Address, H160},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use ansi_term::Colour;

/// Call trace of a tx
#[derive(Clone, Default, Debug, Deserialize, Serialize)]
pub struct CallTrace {
    pub depth: usize,
    pub location: usize,
    /// Successful
    pub success: bool,
    /// Callee
    pub addr: H160,
    /// Creation
    pub created: bool,
    /// Call data, including function selector (if applicable)
    pub data: Vec<u8>,
    /// Gas cost
    pub cost: u64,
    /// Output
    pub output: Vec<u8>,
    /// Logs
    #[serde(skip)]
    pub logs: Vec<RawLog>,
    /// inner calls
    pub inner: Vec<CallTrace>,
}

impl CallTrace {
    pub fn add_trace(&mut self, new_trace: Self) {
        if new_trace.depth == 0 {
            // overwrite
            // self.update(new_trace);
        } else if self.depth == new_trace.depth - 1 {
            self.inner.push(new_trace);
        } else {
            self.inner.last_mut().expect("Disconnected trace").add_trace(new_trace);
        }
    }

    fn update(&mut self, new_trace: Self) {
        self.success = new_trace.success;
        self.addr = new_trace.addr;
        self.cost = new_trace.cost;
        self.output = new_trace.output;
        self.logs = new_trace.logs;
        self.data = new_trace.data;
        self.addr = new_trace.addr;
        // we dont update inner because the temporary new_trace doesnt track inner calls
    }

    pub fn update_trace(&mut self, new_trace: Self) {
        if new_trace.depth == 0 {
            self.update(new_trace);
        } else if self.depth == new_trace.depth - 1 {
            self.inner[new_trace.location].update(new_trace);
        } else {
            self.inner.last_mut().expect("Disconnected trace update").update_trace(new_trace);
        }
    }

    pub fn location(&self, new_trace: &Self) -> usize {
        if new_trace.depth == 0 {
            0
        } else if self.depth == new_trace.depth - 1 {
            self.inner.len()
        } else {
            self.inner.last().expect("Disconnected trace location").location(new_trace)
        }
    }

    pub fn inner_number_of_logs(&self) -> usize {
        // only count child logs
        let mut total = 0;
        if self.inner.len() > 0 {
            self.inner.iter().for_each(|inner| {
                total += inner.inner_number_of_logs();
            });
        }
        total += self.logs.len();
        total
    }

    pub fn inner_number_of_inners(&self) -> usize {
        // only count child logs
        let mut total = 0;
        if self.inner.len() > 0 {
            self.inner.iter().for_each(|inner| {
                total += inner.inner_number_of_inners();
            });
        }
        total += self.inner.len();
        total
    }

    pub fn get_trace(&self, depth: usize, location: usize) -> Option<&CallTrace> {
        if self.depth == depth && self.location == location {
            return Some(&self)
        } else {
            if self.depth != depth {
                for inner in self.inner.iter() {
                    if let Some(trace) = inner.get_trace(depth, location) {
                        return Some(trace)
                    }
                }
            }
        }
        return None
    }

    pub fn pretty_print(
        &self,
        contracts: &BTreeMap<String, (Abi, Address, Vec<String>)>,
        left: String,
    ) {
        if let Some((name, (abi, addr, _other))) =
            contracts.iter().find(|(_key, (_abi, addr, _other))| addr == &self.addr)
        {
            let color = if self.success { Colour::Green } else { Colour::Red };
            // let indent = "\t".repeat(self.depth);
            for (func_name, overloaded_funcs) in abi.functions.iter() {
                for func in overloaded_funcs.iter() {
                    if func.selector() == self.data[0..4] {
                        println!(
                            "{}[{}] {}::{}({:?})",
                            left,
                            self.cost,
                            color.paint(name),
                            color.paint(func_name),
                            func.decode_input(&self.data[4..]).unwrap()
                        );
                    }
                }
            }

            self.inner.iter().enumerate().for_each(|(i, inner)| {
                // let inners = inner.inner_number_of_inners();
                if i == self.inner.len() - 1 && self.logs.len() == 0 {
                    inner.pretty_print(contracts, left.to_string().replace("├─ ", "|  ") + "└─ ");
                } else {
                    inner.pretty_print(contracts, left.to_string().replace("├─ ", "|  ") + "├─ ");
                }
            });

            self.logs.iter().enumerate().for_each(|(i, log)| {
                for (event_name, overloaded_events) in abi.events.iter() {
                    let mut found = false;
                    let mut right = "├─ ";
                    if i == self.logs.len() - 1 {
                        right = "└─ ";
                    }
                    for event in overloaded_events.iter() {
                        if event.signature() == log.topics[0] {
                            found = true;
                            println!(
                                "{}emit {}({})",
                                left.to_string().replace("├─ ", "|  ") + right,
                                Colour::Cyan.paint(event_name),
                                Colour::Cyan
                                    .paint(format!("{:?}", event.parse_log(log.clone()).unwrap()))
                            );
                        }
                    }
                    if !found {
                        println!(
                            "{}emit {}",
                            left.to_string().replace("├─ ", "|  ") + right,
                            Colour::Blue.paint(format!("{:?}", log))
                        )
                    }
                }
            });
        } else {
            if self.data.len() >= 4 {
                println!(
                    "{}{:x}::{}({})",
                    left,
                    self.addr,
                    hex::encode(&self.data[0..4]),
                    hex::encode(&self.data[4..])
                );
            } else {
                println!("{}{:x}::({})", left, self.addr, hex::encode(&self.data));
            }

            self.inner.iter().enumerate().for_each(|(i, inner)| {
                // let inners = inner.inner_number_of_inners();
                if i == self.inner.len() - 1 && self.logs.len() == 0 {
                    inner.pretty_print(contracts, left.to_string().replace("├─ ", "|  ") + "└─ ");
                } else {
                    inner.pretty_print(contracts, left.to_string().replace("├─ ", "|  ") + "├─ ");
                }
            });

            let mut right = "├─ ";

            self.logs.iter().enumerate().for_each(|(i, log)| {
                if i == self.logs.len() - 1 {
                    right = "└─ ";
                }
                println!(
                    "{}emit {}",
                    left.to_string().replace("├─ ", "|  ") + right,
                    Colour::Cyan.paint(format!("{:?}", log))
                )
            });
        }
    }
}