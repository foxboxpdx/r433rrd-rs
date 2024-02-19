use rrdtool::RRDTool;
use tokio::net::UdpSocket;
use serde_derive::Deserialize;
use serde_json::Value;
use std::net::SocketAddr;
use std::error::Error;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use log::{debug, error, warn, info, trace};

pub mod rrdtool;

type Graphmap = HashMap<String, Instant>;

pub struct Server {
    pub socket: UdpSocket,
    pub buf: Vec<u8>,
    pub to_send: Option<(usize, SocketAddr)>,
    pub config: ConfigFile
}

#[derive(Deserialize)]
struct RtlOutput {
    // Fields omitted: time, battery_ok, status, mic
    model: String,
    id: Option<i32>,
    channel: Option<Value>, // can be an int or a string
    #[serde(rename = "temperature_C")]
    temperature_c: Option<f32>,
    humidity: Option<f32>,
    #[serde(rename = "type")]
    ttype: Option<String>
}

struct ParsedSensorData {
    label: String,
    rrd_file: String,
    temperature: String,
    humidity: String 
}

#[derive(Deserialize)]
pub struct ConfigFile {
    pub listen_addr: std::net::SocketAddr,
    pub rrd_path: String,
    pub graph_path: String,
    pub graph_interval: u64,
    pub graph_schedule: Vec<String>
}


impl Server {
    pub async fn run(self) -> Result<(), Box<dyn Error>> {
        let Server { socket, mut buf, mut to_send, config } = self;
        // Initialize a Graphmap for tracking when to call rrdgraph
        let mut g = Graphmap::new();

        loop {
            // If there is a message waiting, send it to the admin
            if let Some((size, _peer)) = to_send {
                let what = match std::str::from_utf8(&buf[..size]){
                    Ok(x) => x,
                    Err(_) => "Error reading string from socket!"
                };
                // At this point the contents of 'what' are the message that was
                // received.  So do stuff with that.
                trace!("{}", what);

                // Make sure this looks like a valid Syslog line, then hand it
                // off to another function to do the work.
                if what.starts_with("<") {
                    match Self::parse(what.to_string(), &config).await {
                        Err(e) => { error!("Encountered an error while parsing: {}", e); },
                        Ok(data) => { 
                            match Self::do_rrd_stuff(&data, &mut g, &config).await {
                                Ok(_) => { debug!("Successfully did_rrd_stuff"); },
                                Err(e) => { error!("Encountered an error doin rrd stuff: {}", e); }
                            }
                        }
                    }
                }
                else {
                    // it's probably that error message
                    warn!("Message from socket doesn't look right, tossing: {}", what);
                }
            }
            // If we're here then `to_send` is `None`, so we take a look for the
            // next message we're going to echo back.
            to_send = Some(socket.recv_from(&mut buf).await?);
        }
    }

    async fn parse(line: String, config: &ConfigFile) -> Result<ParsedSensorData, Box<dyn Error>> {
        // syslog fields:
        // <PRI>VER, timestamp, hostname, command, pid, mid, sdata, payload
        // split on ' ' and pop
        let mut parts: Vec<&str> = line.splitn(8, ' ').collect();
        let payload = match parts.pop() {
            Some(x) => x,
            None => { 
                warn!("Couldn't pop from parts? {:?}", parts);
                return Err("Error splitting input string".into()); 
            }
        };
        trace!("Raw JSON payload: {}", payload);
        let parsed: RtlOutput = match serde_json::from_str(payload) {
            Ok(x) => {
                trace!("Parsed JSON Ok"); 
                x
            },
            Err(e) => {
                return Err(format!("Error parsing JSON Payload: {:?}", e).into()); 
            }
        };
        
        // First, all the stuff we should fail over
        if let Some(tt) = parsed.ttype { 
            if tt == "TPMS" {
                return Err("Skipping entry: TPMS sensor".into());
            }
        }
        let temperature = match parsed.temperature_c {
            Some(x) => x.to_string(),
            None => { return Err("No temperature_c found in syslog data".into());}
        };
        let humidity = match parsed.humidity {
            Some(x) => x.to_string(),
            None => {
                trace!("No humidity value, defaulting to 0.0");
                "0.0".to_string()
            }
        };

        // Sanitize the model name
        let mut label = parsed.model.replace(" ", "_").replace("/", "_").replace(".", "_").replace("&", "");
        
        // Add additonal identifiers - prefer channel, settle for id.
        if let Some(ch) = parsed.channel {
            // Channel was specified, see if it's an i32 or a string
            if let Ok(x) = serde_json::from_value::<i32>(ch.clone()) {
                label = format!("{}.CH{}", label, x);
                trace!("Added integer channel to label");
            }
            else if let Ok(x) = serde_json::from_value::<String>(ch.clone()) {
                label = format!("{}.CH{}", label, x);
                trace!("Added string channel to label");
            }
        }
        else if let Some(id) = parsed.id {
            label = format!("{}.ID{}", label, id);
            trace!("Added ID to label");

        }
        label = format!("{}.rrd", label);
        let rrd_file = format!("{}{}", config.rrd_path, label);
        info!("Data received: {}, {}, {}", label, temperature, humidity);
        Ok(ParsedSensorData { label, rrd_file, temperature, humidity })
    }

    async fn do_rrd_stuff(data: &ParsedSensorData, graph: &mut Graphmap, config: &ConfigFile) -> Result<(), Box<dyn Error>> {
        // See if we need to create this file
        trace!("Calling rrdtool info on {}", data.rrd_file);
        if let Err(_) = RRDTool::info(data.rrd_file.to_string()).await {
            debug!("RRD file for {} did not exist, creating.", &data.label);
            // Yes we do.  Build some args.
            let args: Vec<&str> = vec![
                "--step", "1800", "--start", "0",
                "DS:temperature:GAUGE:2000:U:U",
                "DS:humidity:GAUGE:2000:U:U",
                "RRA:AVERAGE:0.5:1:600",
                "RRA:AVERAGE:0.5:6:700",
                "RRA:AVERAGE:0.5:24:775",
                "RRA:AVERAGE:0.5:288:797",
                "RRA:MAX:0.5:1:600",
                "RRA:MAX:0.5:6:700",
                "RRA:MAX:0.5:24:775",
                "RRA:MAX:0.5:444:797"
            ];
            if let Err(e) = RRDTool::create(data.rrd_file.to_string(), args).await {
                return Err(e);
            }
        }

        // Try to update the RRD file
        let args: Vec<&str> = vec![&data.temperature, &data.humidity];
        if let Err(e) = RRDTool::update(data.rrd_file.to_string(), args).await {
            return Err(e);
        }

        // See if this rrd has been graphed before.  If not, make one now and
        // stash a timestamp of now + GRAPH_INTERVAL along with it.
        // If it exists, see if that timestamp is less than now, and if so, 
        // graph it and update the timestamp.
        let now = Instant::now();
        // sleep for a sec to let that 'now' become 'then'
        std::thread::sleep(Duration::new(1, 0));
        // If this label isn't in the hashmap, insert it
        let when = graph.entry(data.label.to_string()).or_insert(now);
        // WHEN WILL THEN BE NOW?
        // SOON!
        match now.checked_duration_since(*when) {
             // If now is after when, we need a graph
            Some(_) => {
                match Self::make_graphs(data.rrd_file.to_string(), data.label.to_string(), config.graph_path.to_string(), &config.graph_schedule).await {
                    Ok(_) => {
                        // Update the Instant to when the next graph should be made
                        if let Some(x) = now.checked_add(Duration::from_secs(config.graph_interval)) {
                            // HashMap's Entry API lets us dereference 'when' to
                            // update the value.  Neat.
                            *when = x;
                            trace!("Updated next graph time");
                        }
                        else {
                            // If checked_add returns None I don't even know how to
                            // handle that so pretend it didn't happen.
                            warn!("Somehow checked_add returned None.  What.");
                        }
                    },
                    Err(e) => { return Err(e); }
                };
            },
            None => {
                // If 'now' is before 'when', do nothing.
                debug!("Skipping graph for {}", &data.rrd_file);
            }
        };

        Ok(())
    }

    async fn make_graphs(file: String, label: String, path: String, schedule: &Vec<String>) -> Result<(), Box<dyn Error>> {
        info!("Generating graphs for {}", label);
        for sched in schedule {
            let outfile = format!("{}metrics-{}.{}.png", path, sched, label);
            let period = sched.chars().next().unwrap();
            let start = format!("-1{}", period);
            let tdef = format!("DEF:t={}:temperature:AVERAGE", &file);
            let hdef = format!("DEF:h={}:humidity:AVERAGE", &file);
            let args: Vec<&str> = vec![
                "--start", &start,
                "--title", label.as_str(),
                "--vertical-label=C",
                "--right-axis-label=%",
                "-w 800", "-h 200",
                &tdef, &hdef,
                r#"LINE1:t#00FF00:Temperature\t\t"#,
                r#"LINE2:h#0000FF:Humidity\n"#,
                r#"GPRINT:t:AVERAGE:T avg %5.1lf C\t\t"#,
                r#"GPRINT:h:AVERAGE:H avg %5.0lf\n"#,
                r#"GPRINT:t:MAX:T max %5.1lf C\t\t"#,
                r#"GPRINT:h:MAX:H max %5.0lf\n"#
            ];
            match RRDTool::graph(outfile.to_string(), args).await {
                Ok(_) => { debug!("Call to RRDTool Graph succeeded: {}", &outfile); },
                Err(e) => { return Err(e); }
            };
        }
        Ok(())
    }

}