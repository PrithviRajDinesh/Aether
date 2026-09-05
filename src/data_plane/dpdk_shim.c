#include <rte_eal.h>
#include <rte_ethdev.h>
#include <rte_mbuf.h>
#include <stdint.h>


uint16_t aether_eth_rx_burst(
    uint16_t port_id,
    uint16_t queue_id,
    struct rte_mbuf **rx_pkts,
    uint16_t nb_pkts) {

    return rte_eth_rx_burst(
        port_id,
        queue_id,
        rx_pkts,
        nb_pkts
    );
}


uint16_t aether_eth_tx_burst(
    uint16_t port_id,
    uint16_t queue_id,
    struct rte_mbuf **tx_pkts,
    uint16_t nb_pkts) {
    
    return rte_eth_tx_burst(
        port_id,
        queue_id,
        tx_pkts,
        nb_pkts
    );
}


void * aether_pktmbuf_mtod(struct rte_mbuf *mbuf) {
    return rte_pktmbuf_mtod(
        mbuf,
        void *
    );
}


uint32_t aether_pktmbuf_pkt_len(struct rte_mbuf *mbuf) {
    return rte_pktmbuf_pkt_len(mbuf);
}


void aether_pktmbuf_free(struct rte_mbuf *mbuf) {
    rte_pktmbuf_free(mbuf);
}


void aether_mbuf_set_tx_checksum_offload(
    struct rte_mbuf *mbuf,
    uint16_t l2_len,
    uint16_t l3_len,
    uint16_t l4_len) {

    if (mbuf == NULL) {
        return;
    }

    mbuf->l2_len = l2_len;
    mbuf->l3_len = l3_len;
    mbuf->l4_len = l4_len;

    mbuf->ol_flags |=
        RTE_MBUF_F_TX_IPV4 |
        RTE_MBUF_F_TX_IP_CKSUM |
        RTE_MBUF_F_TX_TCP_CKSUM;
}
