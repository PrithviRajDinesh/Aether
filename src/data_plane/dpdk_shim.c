#include <stdint.h>

#include <rte_ethdev.h>
#include <rte_mbuf.h>

uint16_t
aether_eth_rx_burst(
    uint16_t port_id,
    uint16_t queue_id,
    struct rte_mbuf **rx_pkts,
    uint16_t nb_pkts
)
{
    return rte_eth_rx_burst(
        port_id,
        queue_id,
        rx_pkts,
        nb_pkts
    );
}
