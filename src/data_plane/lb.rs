use std::collections::HashMap;

#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug)]
// Communication session key
pub struct FlowKey{
    pub src_ip : u32,
    pub dst_ip : u32,
    pub src_port : u16,
    pub dst_port : u16,
    pub protocol : u8,
}

#[derive(Copy, Clone, Debug)]
pub struct Backend {
    pub ip : u32,
    pub port : u16,
}

pub struct LoadBalancer {
    backends : Vec<Backend>,
    flows : HashMap<FlowKey, usize>,
    next_backend : usize,
}

impl LoadBalancer {
    pub fn new(backends : Vec<Backend>) -> Self {
        assert!(!backends.is_empty(), "Load Balancer requires atleast one backend");

        Self {
            backends,
            flows : HashMap::new(),
            next_backend: 0,
        }
    }

    pub fn lookup(&mut self, flow : FlowKey) -> Backend {
        //If a flow already exists, return assigned backend
        if let Some(&backend_index) = self.flows.get(&flow){
            return self.backends[backend_index];
        }

        //select new backend if it is a new flow
        let backend_index = self.next_backend;

        self.next_backend = (self.next_backend + 1) % self.backends.len();

        self.flows.insert(flow, backend_index);

        self.backends[backend_index]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stateful_lookup() {
        let backends = vec![
            Backend {
                ip: u32::from_be_bytes([10, 0, 0, 10]),
                port: 8080,
            },
            Backend {
                ip: u32::from_be_bytes([10, 0, 0, 11]),
                port: 8080,
            },
            Backend {
                ip: u32::from_be_bytes([10, 0, 0, 12]),
                port: 8080,
            },
        ];

        let mut lb = LoadBalancer::new(backends);

        let flow1 = FlowKey {
            src_ip: u32::from_be_bytes([192, 168, 1, 10]),
            dst_ip: u32::from_be_bytes([10, 0, 0, 100]),
            src_port: 50000,
            dst_port: 80,
            protocol: 6,
        };

        let flow2 = FlowKey {
            src_ip: u32::from_be_bytes([192, 168, 1, 20]),
            dst_ip: u32::from_be_bytes([10, 0, 0, 100]),
            src_port: 50001,
            dst_port: 80,
            protocol: 6,
        };

        let backend1 = lb.lookup(flow1);
        let backend2 = lb.lookup(flow2);

        // New flows should be distributed.
        assert_eq!(backend1.ip, u32::from_be_bytes([10, 0, 0, 10]));
        assert_eq!(backend2.ip, u32::from_be_bytes([10, 0, 0, 11]));

        // The same flow must return the same backend.
        let backend1_again = lb.lookup(flow1);

        assert_eq!(backend1_again.ip, backend1.ip);
        assert_eq!(backend1_again.port, backend1.port);
    }
}

