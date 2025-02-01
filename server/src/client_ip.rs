use std::net::{IpAddr, SocketAddr};

use warp::{filters::path::FullPath, http::Method, reject::Rejection, Filter};

#[derive(Debug)]
pub struct ClientInfo {
    ip: IpAddr,
    source: ClientInfoSource,
}

impl ClientInfo {
    pub fn log(&self, path: &FullPath, method: &Method, correct: bool) {
        if correct {
            log::info!(target: "request", "{} => {} {}", self.ip, method, path.as_str());
        } else {
            log::warn!(target: "request", "{} => {} {}", self.ip, method, path.as_str());
        }
    }
}

#[derive(Debug)]
pub enum ClientInfoSource {
    RemoteAddr,
    XRealIp,
    XForwardedFor,
}

pub fn get_ip() -> impl Filter<Extract = (ClientInfo,), Error = Rejection> + Clone {
    warp::addr::remote()
        .and(warp::header::optional::<String>("x-forwarded-for"))
        .and(warp::header::optional::<String>("x-real-ip"))
        .and_then(
            |remote: Option<SocketAddr>, xff: Option<String>, xri: Option<String>| async move {
                // 尝试从 X-Forwarded-For 获取
                if let Some(ip) = parse_x_forwarded_for(&xff) {
                    return Ok(ClientInfo {
                        ip,
                        source: ClientInfoSource::XForwardedFor,
                    });
                }

                // 尝试从 X-Real-IP 获取
                if let Some(ip) = parse_single_header(&xri) {
                    return Ok(ClientInfo {
                        ip,
                        source: ClientInfoSource::XRealIp,
                    });
                }

                // 回退到 remote_addr
                if let Some(addr) = remote {
                    return Ok(ClientInfo {
                        ip: addr.ip(),
                        source: ClientInfoSource::RemoteAddr,
                    });
                }

                return Err(warp::reject::not_found());

                fn parse_single_header(header: &Option<String>) -> Option<IpAddr> {
                    header.as_ref()?.parse().ok()
                }

                fn parse_x_forwarded_for(xff: &Option<String>) -> Option<IpAddr> {
                    let xff = xff.as_ref()?;
                    xff.split(',').find_map(|s| s.trim().parse().ok())
                }
            },
        )
}

pub fn log_req(correct: bool) -> impl Filter<Extract = (), Error = Rejection> + Clone {
    get_ip()
        .and(warp::path::full())
        .and(warp::method())
        .map(move |ci: ClientInfo, path, method| {
            ci.log(&path, &method, correct);
        })
        .untuple_one()
}
