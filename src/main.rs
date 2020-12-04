use std::io::{self, Read};

use atty::Stream;
use reqwest::blocking::Client;
use reqwest::header::{HeaderValue, ACCEPT, CONTENT_TYPE};
use structopt::StructOpt;
#[macro_use]
extern crate lazy_static;

mod auth;
mod cli;
mod printer;
mod request_items;
mod url;
mod utils;

use auth::Auth;
use cli::{AuthType, Opt, Pretty, RequestItem, Theme};
use printer::Printer;
use request_items::{Body, RequestItems};
use url::Url;

fn body_from_stdin() -> Option<Body> {
    if atty::isnt(Stream::Stdin) {
        let mut buffer = String::new();
        io::stdin().read_to_string(&mut buffer).unwrap();
        Some(Body::Raw(buffer))
    } else {
        None
    }
}

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let opt = Opt::from_args();

    let printer = Printer::new(opt.pretty, opt.theme);
    let request_items = RequestItems::new(opt.request_items);

    let url = Url::new(opt.url, opt.default_scheme);
    let method = opt.method.into();
    let auth = Auth::new(opt.auth, opt.auth_type, &url);
    let query = request_items.query();
    let mut headers = request_items.headers(&url);
    let body = match (
        request_items.body(opt.form, opt.multipart)?,
        body_from_stdin(),
    ) {
        (Some(_), Some(_)) => {
            return Err(
                "Request body (from stdin) and Request data (key=value) cannot be mixed".into(),
            )
        }
        (Some(body), None) | (None, Some(body)) => Some(body),
        (None, None) => None,
    };

    let client = Client::new();
    let request = {
        let mut request_builder = client.request(method, url.0);

        request_builder = match body {
            Some(Body::Form(body)) => request_builder.form(&body),
            Some(Body::Multipart(body)) => request_builder.multipart(body),
            Some(Body::Json(body)) => {
                headers
                    .entry(ACCEPT)
                    .or_insert(HeaderValue::from_static("application/json, */*"));
                request_builder.json(&body)
            }
            Some(Body::Raw(body)) => {
                headers
                    .entry(ACCEPT)
                    .or_insert(HeaderValue::from_static("application/json, */*"));
                headers
                    .entry(CONTENT_TYPE)
                    .or_insert(HeaderValue::from_static("application/json"));
                request_builder.body(body)
            }
            None => request_builder,
        };

        request_builder = match auth {
            Some(Auth::Bearer(token)) => request_builder.bearer_auth(token),
            Some(Auth::Basic(username, password)) => request_builder.basic_auth(username, password),
            None => request_builder,
        };

        request_builder.query(&query).headers(headers).build()?
    };

    print!("\n");

    if opt.verbose {
        printer.print_request_headers(&request);
        printer.print_request_body(&request);
    }

    if !opt.offline {
        let response = client.execute(request)?;
        printer.print_response_headers(&response);
        printer.print_response_body(response);
    }
    Ok(())
}
