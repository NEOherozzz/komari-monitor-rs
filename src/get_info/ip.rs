use crate::config::IpProvider;
use log::trace;
use miniserde::{Deserialize, Serialize, json};
use std::net::{Ipv4Addr, Ipv6Addr};
use std::str::FromStr;
use tokio::task::JoinHandle;

#[cfg(feature = "ureq-support")]
use std::time::Duration;

pub async fn ip(provider: &IpProvider) -> IPInfo {
    match provider {
        IpProvider::Cloudflare => ip_cloudflare().await,
        IpProvider::Ipinfo => ip_ipinfo().await,
    }
}

#[derive(Debug)]
pub struct IPInfo {
    pub ipv4: Option<Ipv4Addr>,
    pub ipv6: Option<Ipv6Addr>,
}

#[derive(Serialize, Deserialize)]
struct IpJson {
    ip: String,
}

// 提取公共的请求函数以减少重复代码
async fn fetch_ipinfo_v4() -> Option<String> {
    #[cfg(feature = "ureq-support")]
    {
        let resp = ureq::get("https://ipinfo.io")
            .header("User-Agent", "curl/8.7.1")
            .config()
            .timeout_global(Some(Duration::from_secs(5)))
            .ip_family(ureq::config::IpFamily::Ipv4Only)
            .build()
            .call();

        if let Ok(mut response) = resp {
            return response.body_mut().read_to_string().ok();
        }
    }

    #[cfg(feature = "nyquest-support")]
    {
        use nyquest::Request;
        let client = crate::utils::create_nyquest_client(false);
        let request = Request::get("https://ipinfo.io");

        if let Ok(res) = client.request(request) {
            return res.text().ok();
        }
    }

    None
}

async fn fetch_ipinfo_v6() -> Option<String> {
    #[cfg(feature = "ureq-support")]
    {
        let resp = ureq::get("https://6.ipinfo.io")
            .header("User-Agent", "curl/8.7.1")
            .config()
            .timeout_global(Some(Duration::from_secs(5)))
            .ip_family(ureq::config::IpFamily::Ipv6Only)
            .build()
            .call();

        if let Ok(mut response) = resp {
            return response.body_mut().read_to_string().ok();
        }
    }

    #[cfg(feature = "nyquest-support")]
    {
        use nyquest::Request;
        let client = crate::utils::create_nyquest_client(false);
        let request = Request::get("https://6.ipinfo.io");

        if let Ok(res) = client.request(request) {
            return res.text().ok();
        }
    }

    None
}

async fn fetch_cloudflare_v4() -> Option<String> {
    #[cfg(feature = "ureq-support")]
    {
        let resp = ureq::get("https://www.cloudflare.com/cdn-cgi/trace")
            .header("User-Agent", "curl/8.7.1")
            .config()
            .timeout_global(Some(Duration::from_secs(5)))
            .ip_family(ureq::config::IpFamily::Ipv4Only)
            .build()
            .call();

        if let Ok(mut response) = resp {
            return response.body_mut().read_to_string().ok();
        }
    }

    #[cfg(feature = "nyquest-support")]
    {
        use nyquest::Request;
        let client = crate::utils::create_nyquest_client(false);
        let request = Request::get("https://1.1.1.1/cdn-cgi/trace");

        if let Ok(res) = client.request(request) {
            return res.text().ok();
        }
    }

    None
}

async fn fetch_cloudflare_v6() -> Option<String> {
    #[cfg(feature = "ureq-support")]
    {
        let resp = ureq::get("https://www.cloudflare.com/cdn-cgi/trace")
            .header("User-Agent", "curl/8.7.1")
            .config()
            .timeout_global(Some(Duration::from_secs(5)))
            .ip_family(ureq::config::IpFamily::Ipv6Only)
            .build()
            .call();

        if let Ok(mut response) = resp {
            return response.body_mut().read_to_string().ok();
        }
    }

    #[cfg(feature = "nyquest-support")]
    {
        use nyquest::Request;
        let client = crate::utils::create_nyquest_client(false);
        let request = Request::get("https://[2606:4700:4700::1111]/cdn-cgi/trace");

        if let Ok(res) = client.request(request) {
            return res.text().ok();
        }
    }

    None
}

fn parse_ipinfo_response(body: &str) -> Option<String> {
    let json: IpJson = json::from_str(body).ok()?;
    Some(json.ip)
}

fn extract_cloudflare_ip(body: &str) -> Option<String> {
    for line in body.lines() {
        if line.starts_with("ip=") {
            return Some(line.replace("ip=", ""));
        }
    }
    None
}

pub async fn ip_ipinfo() -> IPInfo {
    let ipv4: JoinHandle<Option<Ipv4Addr>> = tokio::spawn(async move {
        if let Some(body) = fetch_ipinfo_v4().await {
            if let Some(ip_str) = parse_ipinfo_response(&body) {
                return Ipv4Addr::from_str(&ip_str).ok();
            }
        }
        None
    });

    let ipv6: JoinHandle<Option<Ipv6Addr>> = tokio::spawn(async move {
        if let Some(body) = fetch_ipinfo_v6().await {
            if let Some(ip_str) = parse_ipinfo_response(&body) {
                return Ipv6Addr::from_str(&ip_str).ok();
            }
        }
        None
    });

    let ipv4_result = ipv4.await.unwrap_or(None);
    let ipv6_result = ipv6.await.unwrap_or(None);

    let ip_info = IPInfo {
        ipv4: ipv4_result,
        ipv6: ipv6_result,
    };

    trace!("IP INFO (ipinfo) successfully retrieved: {:?}", ip_info);

    ip_info
}

pub async fn ip_cloudflare() -> IPInfo {
    let ipv4: JoinHandle<Option<Ipv4Addr>> = tokio::spawn(async move {
        if let Some(body) = fetch_cloudflare_v4().await {
            if let Some(ip_str) = extract_cloudflare_ip(&body) {
                return Ipv4Addr::from_str(&ip_str).ok();
            }
        }
        None
    });

    let ipv6: JoinHandle<Option<Ipv6Addr>> = tokio::spawn(async move {
        if let Some(body) = fetch_cloudflare_v6().await {
            if let Some(ip_str) = extract_cloudflare_ip(&body) {
                return Ipv6Addr::from_str(&ip_str).ok();
            }
        }
        None
    });

    let ipv4_result = ipv4.await.unwrap_or(None);
    let ipv6_result = ipv6.await.unwrap_or(None);

    let ip_info = IPInfo {
        ipv4: ipv4_result,
        ipv6: ipv6_result,
    };

    trace!("IP INFO (cloudflare) successfully retrieved: {:?}", ip_info);

    ip_info
}
