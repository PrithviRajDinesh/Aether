use std::io::{
    Read,
    Write,
};

use std::net::{
    TcpListener,
    TcpStream,
};

use std::thread;

use crate::control_plane::telemetry;


pub fn start_dashboard() -> thread::JoinHandle<()> {

    println!(
        "Web Dashboard      : Starting"
    );

    thread::spawn(|| {

        let listener =
            TcpListener::bind(
                "127.0.0.1:8080"
            )
            .expect(
                "Failed to start dashboard server"
            );

        println!(
            "Web Dashboard      : http://localhost:8080"
        );

        for stream in listener.incoming() {

            match stream {

                Ok(stream) => {

                    handle_connection(stream);
                }

                Err(error) => {

                    eprintln!(
                        "Dashboard Connection Error: {}",
                        error
                    );
                }
            }
        }
    })
}


fn handle_connection(
    mut stream: TcpStream,
) {

    let mut buffer = [0_u8; 1024];

    let bytes_read =
        match stream.read(&mut buffer) {

            Ok(bytes) => bytes,

            Err(_) => return,
        };

    let request =
        String::from_utf8_lossy(
            &buffer[..bytes_read]
        );

    let request_line =
        request
            .lines()
            .next()
            .unwrap_or("");

    if request_line.starts_with(
        "GET /metrics"
    ) {

        send_metrics(
            &mut stream
        );

    } else {

        send_dashboard(
            &mut stream
        );
    }
}


fn send_metrics(
    stream: &mut TcpStream,
) {

    let packets_received =
        telemetry::packets_received();

    let packets_transmitted =
        telemetry::packets_transmitted();

    let packets_dropped =
        telemetry::packets_dropped();

    let mbufs_forwarded =
        telemetry::mbufs_forwarded();

    /*
     * Zero-copy rate: every forwarded packet was zero-copy.
     * Express as integer % for JSON simplicity.
     */
    let zero_copy_pct: u64 =
        if packets_received > 0 {
            mbufs_forwarded * 100 / packets_received
        } else {
            0
        };

    let processing_latency =
        telemetry::average_processing_latency_ns();

    let latency_p50 =
        telemetry::latency_p50_ns();

    let latency_p99 =
        telemetry::latency_p99_ns();

    let latency_p999 =
        telemetry::latency_p999_ns();

    let latency_jitter =
        telemetry::latency_jitter_ns();

    let rx_queue_depth_pct =
        telemetry::rx_queue_depth_pct();

    let tx_queue_depth_pct =
        telemetry::tx_queue_depth_pct();

    let body =
        format!(
            "{{\
\"packets_received\":{},\
\"packets_transmitted\":{},\
\"packets_dropped\":{},\
\"mbufs_forwarded\":{},\
\"zero_copy_pct\":{},\
\"processing_latency_ns\":{},\
\"latency_p50_ns\":{},\
\"latency_p99_ns\":{},\
\"latency_p999_ns\":{},\
\"latency_jitter_ns\":{},\
\"rx_queue_depth_pct\":{},\
\"tx_queue_depth_pct\":{}\
}}",

            packets_received,
            packets_transmitted,
            packets_dropped,
            mbufs_forwarded,
            zero_copy_pct,
            processing_latency,
            latency_p50,
            latency_p99,
            latency_p999,
            latency_jitter,
            rx_queue_depth_pct,
            tx_queue_depth_pct,
        );

    let response =
        format!(
            "HTTP/1.1 200 OK\r\n\
Content-Type: application/json\r\n\
Content-Length: {}\r\n\
Connection: close\r\n\
\r\n\
{}",

            body.len(),
            body,
        );

    let _ =
        stream.write_all(
            response.as_bytes()
        );
}


fn send_dashboard(
    stream: &mut TcpStream,
) {

    let html = r#"
<!DOCTYPE html>

<html>

<head>

    <title>Aether Dashboard</title>

    <style>

        * {
            box-sizing: border-box;
        }

        body {

            margin: 0;

            font-family:
                Arial,
                sans-serif;

            background: #111;

            color: #e0e0e0;
        }

        .container {

            width: 860px;

            margin: 60px auto;

            padding: 30px;

            background: #181818;

            border: 1px solid #333;

            border-radius: 8px;
        }

        h1 {

            margin-top: 0;

            color: #4fd1c5;
        }

        .subtitle {

            color: #888;

            margin-bottom: 30px;
        }

        table {

            width: 100%;

            border-collapse: collapse;
        }

        th {

            text-align: left;

            padding: 12px 15px;

            background: #222;

            color: #4fd1c5;
        }

        td {

            padding: 12px 15px;

            border-bottom: 1px solid #2a2a2a;

            font-family: monospace;

            font-size: 15px;
        }

        .value {

            text-align: right;

            color: #ffffff;
        }

        .section-header td {

            background: #1a1a1a;

            color: #4fd1c5;

            font-size: 12px;

            letter-spacing: 0.08em;

            text-transform: uppercase;

            padding: 8px 15px;

            border-bottom: 1px solid #333;
        }

        .status {

            margin-top: 25px;

            color: #66ff99;

            font-size: 14px;
        }

        .bar-container {

            display: inline-block;

            width: 120px;

            background: #2a2a2a;

            border-radius: 3px;

            height: 10px;

            vertical-align: middle;

            margin-right: 8px;
        }

        .bar-fill {

            height: 100%;

            border-radius: 3px;

            background: #4fd1c5;

            width: 0%;
        }

    </style>

</head>


<body>

<div class="container">

    <h1>Aether</h1>

    <div class="subtitle">

        Kernel-Bypass Network Data Plane &mdash; Live Telemetry

    </div>


    <table>

        <thead>

            <tr>

                <th>Metric</th>

                <th style="text-align:right">
                    Value
                </th>

            </tr>

        </thead>


        <tbody>

            <tr class="section-header">
                <td colspan="2">Traffic</td>
            </tr>

            <tr>
                <td>Packets Received</td>
                <td class="value" id="packets_received">0</td>
            </tr>

            <tr>
                <td>Packets Transmitted</td>
                <td class="value" id="packets_transmitted">0</td>
            </tr>

            <tr>
                <td>Packets Dropped</td>
                <td class="value" id="packets_dropped">0</td>
            </tr>

            <tr>
                <td>Current PPS</td>
                <td class="value" id="pps">0</td>
            </tr>


            <tr class="section-header">
                <td colspan="2">Zero-Copy Forwarding</td>
            </tr>

            <tr>
                <td>mbufs Forwarded (zero-copy)</td>
                <td class="value" id="mbufs_forwarded">0</td>
            </tr>

            <tr>
                <td>Zero-Copy Rate</td>
                <td class="value" id="zero_copy_pct">0 %</td>
            </tr>


            <tr class="section-header">
                <td colspan="2">Processing Latency</td>
            </tr>

            <tr>
                <td>Avg Processing Latency</td>
                <td class="value" id="latency">0 ns</td>
            </tr>

            <tr>
                <td>p50 Latency</td>
                <td class="value" id="latency_p50">0 ns</td>
            </tr>

            <tr>
                <td>p99 Latency</td>
                <td class="value" id="latency_p99">0 ns</td>
            </tr>

            <tr>
                <td>p999 Latency</td>
                <td class="value" id="latency_p999">0 ns</td>
            </tr>

            <tr>
                <td>Jitter (p99 &minus; p50)</td>
                <td class="value" id="latency_jitter">0 ns</td>
            </tr>


            <tr class="section-header">
                <td colspan="2">Pipeline Back-Pressure</td>
            </tr>

            <tr>
                <td>RX Queue Utilisation</td>
                <td class="value" id="rx_queue_depth_pct">
                    <span class="bar-container">
                        <span class="bar-fill" id="rx_bar"></span>
                    </span>
                    0 %
                </td>
            </tr>

            <tr>
                <td>TX Queue Utilisation</td>
                <td class="value" id="tx_queue_depth_pct">
                    <span class="bar-container">
                        <span class="bar-fill" id="tx_bar"></span>
                    </span>
                    0 %
                </td>
            </tr>

        </tbody>

    </table>


    <div class="status">

        &#9679; Connected to Aether telemetry

    </div>

</div>


<script>

    let previousPackets = 0;

    let previousTime = Date.now();


    function setBar(barId, pct) {
        const bar = document.getElementById(barId);
        if (bar) bar.style.width = pct + "%";
    }


    async function updateMetrics() {

        try {

            const response =
                await fetch(
                    "/metrics"
                );

            const data =
                await response.json();


            document
                .getElementById("packets_received")
                .textContent = data.packets_received;

            document
                .getElementById("packets_transmitted")
                .textContent = data.packets_transmitted;

            document
                .getElementById("packets_dropped")
                .textContent = data.packets_dropped;

            document
                .getElementById("mbufs_forwarded")
                .textContent = data.mbufs_forwarded;

            document
                .getElementById("zero_copy_pct")
                .textContent = data.zero_copy_pct + " %";

            document
                .getElementById("latency")
                .textContent = data.processing_latency_ns + " ns";

            document
                .getElementById("latency_p50")
                .textContent = data.latency_p50_ns + " ns";

            document
                .getElementById("latency_p99")
                .textContent = data.latency_p99_ns + " ns";

            document
                .getElementById("latency_p999")
                .textContent = data.latency_p999_ns + " ns";

            document
                .getElementById("latency_jitter")
                .textContent = data.latency_jitter_ns + " ns";

            document
                .getElementById("rx_queue_depth_pct")
                .innerHTML =
                    '<span class="bar-container"><span class="bar-fill" id="rx_bar"></span></span>'
                    + data.rx_queue_depth_pct + " %";

            document
                .getElementById("tx_queue_depth_pct")
                .innerHTML =
                    '<span class="bar-container"><span class="bar-fill" id="tx_bar"></span></span>'
                    + data.tx_queue_depth_pct + " %";

            setBar("rx_bar", data.rx_queue_depth_pct);
            setBar("tx_bar", data.tx_queue_depth_pct);


            const now =
                Date.now();

            const timeDiff =
                (now - previousTime) / 1000;

            const packetDiff =
                data.packets_received - previousPackets;

            const pps =
                timeDiff > 0
                    ? Math.round(packetDiff / timeDiff)
                    : 0;

            document
                .getElementById("pps")
                .textContent = pps;

            previousPackets = data.packets_received;

            previousTime = now;

        }

        catch (error) {

            console.error(
                "Failed to fetch metrics:",
                error
            );
        }
    }


    updateMetrics();


    setInterval(
        updateMetrics,
        1000
    );


</script>


</body>

</html>
"#;

    let response =
        format!(
            "HTTP/1.1 200 OK\r\n\
Content-Type: text/html\r\n\
Content-Length: {}\r\n\
Connection: close\r\n\
\r\n\
{}",

            html.len(),
            html,
        );

    let _ =
        stream.write_all(
            response.as_bytes()
        );
}
