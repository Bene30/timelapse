use chrono::Utc;
use log::*;
use serde_json::json;
use std::path::Path;
use std::{fs, io, path::PathBuf, thread, time};

use actix_web::{get, http::StatusCode, middleware, App, HttpResponse, HttpServer};
use clap::Clap;

#[derive(Clap, Clone)]
#[clap(version = "1.0")]
struct Opts {
    /// Path to the place to store the timelapse images
    #[clap(short, long, env = "IMAGE_DIR", default_value = "data")]
    image_dir: PathBuf,

    /// The address to listen on
    #[clap(short, long, env = "ADDR", default_value = "127.0.0.1:8181")]
    address: String,
}

#[get("/api")]
async fn api() -> HttpResponse {
    HttpResponse::build(StatusCode::OK)
        .content_type("text/json; charset=utf-8")
        .body("{}")
}


#[get("/api/days")]
async fn days_with_photos() -> HttpResponse {
    let response = json!(dirs_with_images(&Opts::parse().image_dir).unwrap());
    HttpResponse::build(StatusCode::OK)
        .content_type("text/json; charset=utf-8")
        .body(response)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info,actix_server=info,actix_web=info");
    }
    env_logger::init();

    info!("Starting camera loop");
    thread::spawn(move || loop {
        let image_name = format!("{}.jpg", Utc::now().to_rfc3339());
        let image_dir = Opts::parse()
            .image_dir
            .clone()
            .join(&format!("{}", Utc::today()));
        fs::create_dir_all(&image_dir).unwrap();
        let status = std::process::Command::new("raspistill")
            .args(&[
                "-t",
                "1000",
                "-ss",
                "10000",
                "--awb",
                "greyworld",
                "-o",
                image_dir.join(&image_name).to_str().unwrap(),
            ])
            .status();
        match status {
            Ok(status) => match status.code() {
                Some(code) if !status.success() => {
                    error!("raspistill exited with exit status {}", code)
                }
                None => error!("raspistill terminated by signal"),
                _ => info!("captured image {}", image_name),
            },
            Err(err) => error!("{}", err),
        }
        thread::sleep(time::Duration::from_secs(60));
    });

    let opts: Opts = Opts::parse();
    let image_dir = opts.image_dir.clone();
    info!("Starting webserver on {}", &opts.address);
    HttpServer::new(move || {
        App::new()
            .wrap(middleware::DefaultHeaders::new().header("X-Version", "0.2"))
            .wrap(middleware::Compress::default())
            .wrap(middleware::Logger::default())
            .service(actix_files::Files::new("/images", &image_dir).show_files_listing())
            .service(api)
            .service(days_with_photos)
    })
    .bind(opts.address)?
    .workers(1)
    .run()
    .await
}

fn dirs_with_images(image_dir: &Path) -> io::Result<Vec<String>> {
    let mut dirs = Vec::new();
    if image_dir.is_dir() {
        for entry in fs::read_dir(image_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() && fs::read_dir(path)?.count() > 0 {
                dirs.push(entry.file_name().into_string().unwrap());
            }
        }
    };
    Ok(dirs)
}
