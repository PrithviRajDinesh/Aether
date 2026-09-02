#include <rte_eal.h>
#include <rte_ethdev.h>
#include <rte_mbuf.h>
#include <rte_memcpy.h>
#include <rte_version.h>

void aether_mbuf_set_tx_checksum_offload(
    struct rte_mbuf *mbuf,
    uint16_t l2_len,
    uint16_t l3_len,
    uint16_t l4_len
);

uint16_t aether_eth_rx_burst(
    uint16_t port_id,
    uint16_t queue_id,
    struct rte_mbuf **rx_pkts,
    uint16_t nb_pkts
);

void *aether_pktmbuf_mtod(struct rte_mbuf *mbuf);

uint32_t aether_pktmbuf_pkt_len(struct rte_mbuf *mbuf);
