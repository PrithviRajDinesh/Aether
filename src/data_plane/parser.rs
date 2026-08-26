use crate::dpdk;

#[repr(C, packed)]
pub struct EthernetHeader {
    pub dst_addr: [u8; 6],
    pub src_addr: [u8; 6],
    pub ether_type: u16,
}

#[repr(C, packed)]
pub struct Ipv4Header {
    pub version_ihl: u8,
    pub dscp_ecn: u8,
    pub total_length: u16,
    pub identification: u16,
    pub fragment_offset: u16,
    pub ttl: u8,
    pub next_protocol: u8,
    pub header_checksum: u16,
    pub src_addr: u32,
    pub dst_addr: u32,
}

#[repr(C, packed)]
pub struct TcpHeader {
    pub src_port: u16,
    pub dst_port: u16,
    pub sequence_number: u32,
    pub acknowledgment_number: u32,
    pub data_offset_flags: u16,
    pub window_size: u16,
    pub checksum: u16,
    pub urgent_pointer: u16,
}

#[repr(C, packed)]
pub struct UdpHeader {
    pub src_port: u16,
    pub dst_port: u16,
    pub length: u16,
    pub checksum: u16,
}

// Verifying the fixed-size portions of the wire headers.

const _: () = assert!(
    std::mem::size_of::<EthernetHeader>() == 14
);

const _: () = assert!(
    std::mem::size_of::<Ipv4Header>() == 20
);

const _: () = assert!(
    std::mem::size_of::<TcpHeader>() == 20
);

const _: () = assert!(
    std::mem::size_of::<UdpHeader>() == 8
);

pub unsafe fn packet_data_ptr(mbuf: *mut dpdk::rte_mbuf,) -> *mut u8 {
    dpdk::aether_pktmbuf_mtod(mbuf) as *mut u8
}

pub unsafe fn parse_ethernet(mbuf : *mut dpdk::rte_mbuf) -> Option<EthernetHeader> {
    if mbuf.is_null() {
        return None;
    }
    let packet_len = dpdk::aether_pktmbuf_pkt_len(mbuf);

    //Check if EthernetHeader is 14 bytes
    if packet_len < std::mem::size_of::<EthernetHeader>() as u32 {
        return None;
    }

    //Get pointer to packet data
    let data_ptr = packet_data_ptr(mbuf);

    if data_ptr.is_null(){
        return None;
    }

    //EthernetHeader might not be aligned
    let ethernet_header = std::ptr::read_unaligned(data_ptr as *const EthernetHeader);

    Some(ethernet_header)
}
