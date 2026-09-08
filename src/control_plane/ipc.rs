use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

use crate::control_plane::telemetry;

pub fn start_dashboard() -> thread::JoinHandle<()> {
    println!("Web Dashboard      : Starting");

    thread::spawn(|| {
        let listener = TcpListener::bind("127.0.0.1:8080")
            .expect("Failed to start web dashboard");

        println!("Web Dashboard      : http://127.0.0.1:8080");

        for stream in listener.incoming() {
            match stream {
                Ok(mut stream) => {
                    handle_connection(&mut stream);
                }

                Err(error) => {
                    eprintln!(
                        "Web Dashboard Error: {}",
                        error
                    );
                }
            }
        }
    })
}

fn handle_connection(
    stream: &mut std::net::TcpStream,
) {
    let mut buffer = [0_u8; 1024];

    let _ = stream.read(&mut buffer);

    let packets_received =
        telemetry::packets_received();

    let packets_dropped =
        telemetry::packets_dropped();

    let processing_latency =
        telemetry::processing_latency_ns();

    let response_body = format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>Aether Dashboard</title>

    <meta http-equiv="refresh" content="1">

    <style>
        body {{
            font-family: Arial, sans-serif;
            background: #111;
            color: #eee;
            margin: 40px;
        }}

        h1 {{
            color: #00d4ff;
        }}

        table {{
            border-collapse: collapse;
            width: 600px;
            margin-top: 30px;
        }}

        th, td {{
            border: 1px solid #444;
            padding: 15px;
            text-align: left;
        }}

        th {{
            background: #222;
        }}

        td {{
            font-size: 20px;
        }}

        .value {{
            color: #00ff88;
            font-family: monospace;
        }}
    </style>

</head>

<body>

    <h1>Aether Network Data Plane</h1>

    <table>

        <tr>
            <th>Metric</th>
            <th>Value</th>
        </tr>

        <tr>
            <td>Packets Received</td>
            <td class="value">{}</td>
        </tr>

        <tr>
            <td>Packets Dropped</td>
            <td class="value">{}</td>
        </tr>

        <tr>
            <td>Processing Latency</td>
            <td class="value">{} ns</td>
        </tr>

    </table>

    <p>
        Dashboard refreshes automatically every second.
    </p>

</body>
</html>"#,
        packets_received,
        packets_dropped,
        processing_latency,
    );

    let response = format!(
        "HTTP/1.1 200 OK\r\n\
Content-Type: text/html\r\n\
Content-Length: {}\r\n\
Connection: close\r\n\
\r\n\
{}",
        response_body.len(),
        response_body,
    );

    let _ = stream.write_all(response.as_bytes());

    let _ = stream.flush();
}
