use async_process::Command;
use std::error::Error;
use std::str::from_utf8;
use log::trace;

pub struct RRDTool;

impl RRDTool {
    pub async fn create(rrd_file: String, args: Vec<&str>) -> Result<(), Box<dyn Error>> {
        let result = Command::new("rrdtool")
            .arg("create")
            .arg(&rrd_file)
            .args(&args)
            .output()
            .await;

        match result {
            Ok(x) => {
                if x.status.success() {
                    trace!("Created file: {}", &rrd_file);
                }
                else {
                    let reason = format!("stdout: {}\nstderr: {}", from_utf8(&x.stdout)?, from_utf8(&x.stderr)?);
                    return Err(format!("Error creating file:\n{}", reason).into());
                }
            },
            Err(e) => { return Err(Box::new(e)); }
        };

        Ok(())
    }

    pub async fn info(rrd_file: String) -> Result<String, Box<dyn Error>> {
        let result = Command::new("rrdtool").arg("info").arg(&rrd_file).output().await;
        match result {
            Ok(x) => {
                if x.status.success() {
                    let retval = from_utf8(&x.stdout)?;
                    trace!("Call to rrdtool info was successful for: {}", &rrd_file);
                    Ok(retval.to_string())
                }
                else {
                    trace!("Call to rrdtool info returned nonzero for: {}", &rrd_file);
                    let reason = format!("stdout: {}\nstderr: {}", from_utf8(&x.stdout)?, from_utf8(&x.stderr)?);
                    trace!("Error was: {}", reason);
                    Err(format!("Error calling rrdtool info on {}", rrd_file).into())
                }
            },
            Err(e) => Err(Box::new(e))
        }
    }

    pub async fn update(rrd_file: String, values: Vec<&str>) -> Result<(), Box<dyn Error>> {
        let combined = values.join(":");
        let result = Command::new("rrdtool")
            .arg("update")
            .arg(&rrd_file)
            .arg(format!("N:{}",combined))
            .output()
            .await;
        match result {
            Ok(x) => {
                if x.status.success() {
                    trace!("Updated file: {}", rrd_file);
                }
                else {
                    trace!("Call to rrdtool update returned nonzero for: {}", &rrd_file);
                    let reason = format!("stdout: {}\nstderr: {}", from_utf8(&x.stdout)?, from_utf8(&x.stderr)?);
                    return Err(format!("Error Updating file:\n{}", reason).into());
                }
            },
            Err(e) => { return Err(Box::new(e)); }
        }
        Ok(())
    }

    pub async fn graph(outfile: String, args: Vec<&str>) -> Result<(), Box<dyn Error>> {
        let result = Command::new("rrdtool")
        .arg("graph")
        .arg(&outfile)
        .args(&args)
        .output()
        .await;

    match result {
        Ok(x) => {
            if x.status.success() {
                trace!("Created Graph: {}", &outfile);
            }
            else {
                trace!("Call to rrdtool graph returned nonzero for: {}", &outfile);
                let reason = format!("stdout: {}\nstderr: {}", from_utf8(&x.stdout)?, from_utf8(&x.stderr)?);
                return Err(format!("Error creating graph:\n{}", reason).into());
            }
        },
        Err(e) => { return Err(Box::new(e)); }
    };
        Ok(())
    }

}