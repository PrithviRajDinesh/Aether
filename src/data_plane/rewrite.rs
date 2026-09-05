use crate::data_plane::parser::{
    EthernetHeader,
    Ipv4Header,
    TcpHeader,
};
use crate::dpdk;

const ETHERTYPE_IPV4: u16 = 0x0800;
const IP_PROTOCOL_TCP: u8 = 6;

pub unsafe fn rewrite_ipv4_tcp_destination(
    mbuf: *mut dpdk::rte_mbuf,
    dst_ip: u32,
    dst_port: u16,
) -> bool {
    // Make sure the mbuf is valid.
    if mbuf.is_null() {
        return false;
    }

    // Get total packet length.
    let packet_len =
        dpdk::aether_pktmbuf_pkt_len(mbuf) as usize;

    let eth_len =
        std::mem::size_of::<EthernetHeader>();

    // Packet must contain an Ethernet header.
    if packet_len < eth_len {
        return false;
    }

    // Get pointer to the beginning of packet data.
    let data_ptr =
        dpdk::aether_pktmbuf_mtod(mbuf) as *mut u8;

    if data_ptr.is_null() {
        return false;
    }

    // Read Ethernet header without assuming alignment.
    let ethernet_header =
        std::ptr::read_unaligned(
            data_ptr as *const EthernetHeader
        );

    // EtherType is stored in network byte order.
    let ether_type =
        u16::from_be(ethernet_header.ether_type);

    if ether_type != ETHERTYPE_IPV4 {
        return false;
    }

    // IPv4 starts immediately after Ethernet.
    let ipv4_ptr =
        data_ptr.add(eth_len) as *mut Ipv4Header;

    let ipv4_min_len =
        std::mem::size_of::<Ipv4Header>();

    // Make sure the fixed IPv4 header exists.
    if packet_len < eth_len + ipv4_min_len {
        return false;
    }

    let ipv4_header =
        std::ptr::read_unaligned(
            ipv4_ptr as *const Ipv4Header
        );

    let version =
        ipv4_header.version_ihl >> 4;

    if version != 4 {
        return false;
    }

    let ihl =
        (ipv4_header.version_ihl & 0x0f) as usize;

    let ipv4_header_len = ihl * 4;

    // Minimum IPv4 header is 20 bytes.
    if ipv4_header_len < 20 {
        return false;
    }

    // Make sure the complete IPv4 header exists.
    if packet_len < eth_len + ipv4_header_len {
        return false;
    }

    if ipv4_header.next_protocol != IP_PROTOCOL_TCP {
        return false;
    }

    // TCP starts immediately after the IPv4 header.
    let tcp_offset =
        eth_len + ipv4_header_len;

    let tcp_min_len =
        std::mem::size_of::<TcpHeader>();

    if packet_len < tcp_offset + tcp_min_len {
        return false;
    }

    let tcp_ptr = data_ptr.add(tcp_offset) as *mut TcpHeader;

    // Read TCP header to determine its length.
    let tcp_header = std::ptr::read_unaligned(tcp_ptr as *const TcpHeader);

    // TCP data offset occupies the upper 4 bits.
    let tcp_data_offset = (u16::from_be(tcp_header.data_offset_flags) >> 12) as usize;

    let tcp_header_len = tcp_data_offset * 4;

    // Minimum TCP header length is 20 bytes.
    if tcp_header_len < 20 {
        return false;
    }

    // Make sure the complete TCP header exists.
    if packet_len < tcp_offset + tcp_header_len {
        return false;
    }
    // IN-PLACE DESTINATION IP REWRITE
    std::ptr::write_unaligned(
        std::ptr::addr_of_mut!((*ipv4_ptr).dst_addr),
        dst_ip.to_be(),
    );

    std::ptr::write_unaligned(
        std::ptr::addr_of_mut!((*tcp_ptr).dst_port),
        dst_port.to_be(),
    );

    dpdk::aether_mbuf_set_tx_checksum_offload(
        mbuf,
        eth_len as u16,
        ipv4_header_len as u16,
        tcp_header_len as u16,
    );
    true
}
