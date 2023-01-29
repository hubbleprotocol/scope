use serde::Serialize;
use warp::Filter;

pub fn routes() -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    healthcheck()
}

fn healthcheck() -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path("health")
        .and(warp::get())
        .map(|| warp::reply::json(&Healthcheck::up()))
}

#[derive(Serialize)]
struct Healthcheck {
    status: HealthcheckStatus,
}

#[derive(Serialize)]
enum HealthcheckStatus {
    UP,
}

impl Healthcheck {
    pub fn up() -> Healthcheck {
        Healthcheck {
            status: HealthcheckStatus::UP,
        }
    }
}
