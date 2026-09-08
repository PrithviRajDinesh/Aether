#include <rte_eal.h>
#include <rte_ethdev.h>
#include <rte_mbuf.h>
#include <rte_cycles.h>
#include <rte_version.h>

/*
 * Aether DPDK shim functions.
 *
 * Many DPDK "hot path" functions (rx/tx burst, mtod, pkt_len, alloc, etc.)
 * are `static inline` in the DPDK headers, which means they have no
 * linkable symbol and cannot be called directly through Rust FFI.
 * These shims give them a real, exported symbol.
 */

uint64_t aether_rdtsc(void);

uint64_t aether_get_tsc_hz(void);

void aether_pktmbuf_free(
    struct rte_mbuf *mbuf
);

struct rte_mbuf *aether_pktmbuf_alloc(
    struct rte_mempool *mp
);

uint16_t aether_eth_rx_burst(
    uint16_t port_id,
    uint16_t queue_id,
    struct rte_mbuf **rx_pkts,
    uint16_t nb_pkts
);

uint16_t aether_eth_tx_burst(
    uint16_t port_id,
    uint16_t queue_id,
    struct rte_mbuf **tx_pkts,
    uint16_t nb_pkts
);

void *aether_pktmbuf_mtod(
    struct rte_mbuf *mbuf
);

uint32_t aether_pktmbuf_pkt_len(
    struct rte_mbuf *mbuf
);

void aether_mbuf_set_tx_checksum_offload(
    struct rte_mbuf *mbuf,
    uint16_t l2_len,
    uint16_t l3_len,
    uint16_t l4_len
);
