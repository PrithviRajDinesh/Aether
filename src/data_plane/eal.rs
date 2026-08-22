use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;
use crate::dpdk;

pub fn init_hardware_env() {
    // DPDK arguments for testing EAL without any PCI devices
    // or virtual TAP devices.
    let dpdk_args = vec![
        "aether",
        "--no-pci",
        "--vdev=net_tap0,iface=dtap0",
    ];

    // Convert Rust strings into null-terminated C strings.
    let c_strings: Vec<CString> = dpdk_args
        .iter()
        .map(|&arg| CString::new(arg).expect("Failed to convert argument to CString"))
        .collect();

    // Extract raw pointers for the DPDK C API.
    let mut c_ptrs: Vec<*mut c_char> = c_strings
        .iter()
        .map(|c_str| c_str.as_ptr() as *mut c_char)
        .collect();

    let argc = c_ptrs.len() as i32;
    let argv = c_ptrs.as_mut_ptr();

    // Initialize DPDK EAL.
    let ret = unsafe {
        let version_ptr = dpdk::rte_version();

        println!(
            "DPDK Version Found : {}",
            CStr::from_ptr(version_ptr).to_string_lossy()
        );

        dpdk::rte_eal_init(argc, argv)
    };

    if ret < 0 {
        panic!("Fatal: Failed to initialize DPDK EAL.");
    }

    println!(
        "EAL Boot Status    : SUCCESS (Parsed {} arguments)",
        ret
    );

    //Primary packet buffer pool
    let pool_name = CString::new("Aether_MBUF_POOL")
        .expect("Failed to create mbuf pool name");

    let nmbuf : u32 = 8192;
    let cache_size : u32 = 256;
    let priv_size : u16 = 0;
    let data_room_size : u16 = dpdk::RTE_MBUF_DEFAULT_BUF_SIZE as u16;
    let socket_id : i32 = 0;
    
    let mbuf_pool = unsafe{
        dpdk::rte_pktmbuf_pool_create(
            pool_name.as_ptr(),
            nmbuf,
            cache_size,
            priv_size,
            data_room_size,
            socket_id,
        )
    };
    
    if mbuf_pool == ptr::null_mut() {
        panic!("Fatal: Failed to create Aether mbuf pool.");
    }
    println!("MBUF Pool Status     : SUCCESS ({} mbufs)", nmbuf);
    
    //Ethernet port count
    let port_count = unsafe{
        dpdk::rte_eth_dev_count_avail()
    };

    if port_count == 0{
        panic!("Failed, No Available Ethernet Ports Found.");
    }
    println!("Available Ports   : {}", port_count);

    let port_id : u16 = 0;
    let nb_rx_queue : u16 = 1;
    let nb_tx_queue : u16 = 1;

    let port_conf : dpdk::rte_eth_conf = unsafe{
        std::mem::zeroed()
    };

    let ret = unsafe{
        dpdk::rte_eth_dev_configure(
            port_id,
            nb_rx_queue,
            nb_tx_queue,
            &port_conf,
        )
    };

    if ret < 0 {
        panic!(
            "Fatal: Failed to configure Ethernet port {}. Error code: {}", port_id,
            ret
        );
    }

    println!("Port Configuration : SUCCESS (port {}, RX queues {}, TX queues {})",
        port_id,
        nb_rx_queue,
        nb_tx_queue,
    );
    
    //RX Queue Configuration
    let rx_queue_id : u16 = 0;
    let nb_rx_desc : u16 = 1024;
    let rx_socket_id : u32 = 0;

    let rx_conf : dpdk::rte_eth_rxconf = unsafe{
        std::mem::zeroed()
    };

    let ret = unsafe{
        dpdk::rte_eth_rx_queue_setup(
            port_id,
            rx_queue_id,
            nb_rx_queue,
            rx_socket_id,
            &rx_conf,
            mbuf_pool,
        )
    };

    if ret < 0 {
        panic!("Fatal: Failed to configure RX queue {} on Port {}. Error code: {}",
            rx_queue_id,
            port_id,
            ret
        );
    }

    println!("RX Queue Setup    : SUCCESS (queue {}, {} descriptors)",
        rx_queue_id,
        nb_rx_desc
    );

    //TX Queue Configuration
    let tx_queue_id : u16 = 0;
    let nb_tx_desc : u16 = 1024;
    let tx_socket_id : u32 = 0;

    let tx_conf : dpdk::rte_eth_txconf = unsafe{
        std::mem::zeroed()
    };

    let ret = unsafe{
        dpdk::rte_eth_tx_queue_setup(
            port_id,
            tx_queue_id,
            nb_tx_desc,
            tx_socket_id,
            &tx_conf,
        )
    };

    if ret < 0 {
        panic!("Fatal: Failed to configure TX queue {} on Port {}. Error code: {}",
            tx_queue_id,
            port_id,
            ret
        );
    }

    println!("TX Queue Setup    : SUCCESS (queue {}, {} descriptors)",
        tx_queue_id,
        nb_tx_desc
    );

    //Starting the Ethernet device
    let ret = unsafe{
        dpdk::rte_eth_dev_start(port_id)
    };

    if ret < 0 {
        panic!(
            "Fatal: Failed to start Ethernet port {}. Error code: {}",
            port_id,
            ret
        );
    }

    println!(
        "Port Start             : SUCCESS (port {})",
        port_id
    );

    //Promiscuous Mode
    unsafe{
        dpdk::rte_eth_promiscuous_enable(port_id)
    };

    println!("Promiscuous Mode     : Enabled  (port {})", port_id);

    println!("=========================================");
}
