use std::{fs::read_to_string, thread::sleep, time::Duration};

use reqwest::blocking::{ClientBuilder, get};
use serde::{Deserialize, Serialize};
use toml::from_str;

#[derive(Deserialize)]
struct Config {
    pub interval: Option<i32>,
    pub name: Option<String>,
    pub ttl: Option<i32>,
    pub zone: String,
    pub token: String,
}

#[derive(Deserialize)]
struct Zone {
    pub result: ZoneResult,
}

#[derive(Deserialize)]
struct ZoneResult {
    pub name: String,
}

#[derive(Deserialize)]
struct DNSRecords {
    pub result: Vec<DNSRecordsResult>,
}

#[derive(Deserialize)]
struct DNSRecordsResult {
    pub id: String,
}

#[derive(Serialize)]
struct CreateDNSRecord {
    pub name: String,
    pub ttl: i32,
    #[serde(rename = "type")]
    pub record_type: String,
    pub content: String,
}

#[derive(Deserialize)]
struct DNSRecord {
    pub result: DNSRecordResult,
}

#[derive(Deserialize)]
struct DNSRecordResult {
    pub id: String,
}

fn main() {
    let content = read_to_string("config.toml").expect("Failed to read config.toml!");
    let config: Config = from_str(&content).expect("Failed to parse config.toml!");

    let client = ClientBuilder::new()
        .build()
        .expect("Failed to create client!");

    let name = if config.name.clone().unwrap_or("".to_string()).is_empty() {
        let res = client
            .get(format!(
                "https://api.cloudflare.com/client/v4/zones/{}",
                config.zone
            ))
            .bearer_auth(config.token.clone())
            .send()
            .expect("Failed to get domain name!");
        match res.status().as_u16() {
            200 => (),
            403 => panic!("Token does not have DNS edit permissions!"),
            e => panic!("Failed to query domain name with status: {}!", e),
        }
        let zone: Zone = res
            .json()
            .expect("Failed to deserialize domain name request!");
        zone.result.name
    } else {
        config.name.unwrap()
    };

    let res = client
        .get(format!(
            "https://api.cloudflare.com/client/v4/zones/{}/dns_records?type=A&name.startswith={}",
            config.zone, name
        ))
        .bearer_auth(config.token.clone())
        .send()
        .expect("Failed to get DNS record!");
    match res.status().as_u16() {
        200 => (),
        403 => panic!("Token does not have DNS edit permissions!"),
        e => panic!("Failed to query DNS records with status: {}!", e),
    }

    let mut ip = get_ip().expect("Failed to get IP!");

    let dns_records: DNSRecords = res
        .json()
        .expect("Failed to deserialize DNS record request!");

    let dns_id = if dns_records.result.is_empty() {
        let data = CreateDNSRecord {
            name: name.clone(),
            ttl: config.ttl.unwrap_or(60),
            record_type: "A".to_string(),
            content: ip.clone(),
        };

        let res = client
            .post(format!(
                "https://api.cloudflare.com/client/v4/zones/{}/dns_records",
                config.zone
            ))
            .bearer_auth(config.token.clone())
            .json(&data)
            .send()
            .expect("Failed to create DNS record!");
        match res.status().as_u16() {
            200 => (),
            403 => panic!("Token does not have DNS edit permissions!"),
            e => panic!("Failed to create DNS record with status: {}!", e),
        }
        let dns_record: DNSRecord = res
            .json()
            .expect("Failed to deserialize create DNS record request!");
        dns_record.result.id
    } else {
        dns_records.result[0].id.clone()
    };

    loop {
        sleep(Duration::from_secs(config.interval.unwrap_or(10) as u64));
        let Some(new_ip) = get_ip() else {
            println!("Failed to get ip.");
            continue;
        };
        if new_ip != ip {
            println!("New ip found!");
            ip = new_ip;
            let data = CreateDNSRecord {
                name: name.clone(),
                ttl: config.ttl.unwrap_or(60),
                record_type: "A".to_string(),
                content: ip.clone(),
            };
            let res = client
                .patch(format!(
                    "https://api.cloudflare.com/client/v4/zones/{}/dns_records/{}",
                    config.zone, dns_id
                ))
                .bearer_auth(config.token.clone())
                .json(&data)
                .send()
                .expect("Failed to update DNS record!");
            match res.status().as_u16() {
                200 => (),
                403 => panic!("Token does not have DNS edit permissions!"),
                e => panic!("Failed to edit DNS record with status: {}!", e),
            }
        }
    }
}

fn get_ip() -> Option<String> {
    Some(get("https://ipv4.icanhazip.com/").ok()?.text().ok()?)
}
