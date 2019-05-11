#![warn(clippy::pedantic)]
#![allow(clippy::default_trait_access)]
#![allow(clippy::use_self)]

use aws_lambda_events::event::s3::S3Event;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use flate2::{read::GzDecoder, write::GzEncoder};
use futures::{future::join_all, Future};
use lambda_runtime::lambda;
use lazy_static::lazy_static;
use rusoto_s3::{DeleteObjectRequest, GetObjectRequest, PutObjectRequest, S3Client, S3 as _};
use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Write};
use url::Url;

struct LogLine {
    timestamp: NaiveDateTime,
    url: Url,
}

impl LogLine {
    fn parse(line: &str) -> Option<LogLine> {
        if line.starts_with('#') {
            return None;
        }
        let mut iter = line.split('\t');

        let date = match NaiveDate::parse_from_str(iter.next()?, "%Y-%m-%d") {
            Ok(date) => date,
            Err(_) => return None,
        };
        let time = match NaiveTime::parse_from_str(iter.next()?, "%H:%M:%S") {
            Ok(date) => date,
            Err(_) => return None,
        };
        let timestamp = NaiveDateTime::new(date, time);

        // cs-method
        if iter.nth(3)? != "GET" {
            return None;
        }

        // cs-uri-stem
        if iter.nth(1)? != "/pixel.gif" {
            return None;
        }

        let url = match Url::parse(iter.nth(1)?) {
            Ok(url) => url,
            Err(_) => return None,
        };

        Some(LogLine { timestamp, url })
    }

    fn key(&self) -> NaiveDate {
        self.timestamp.date()
    }

    fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        writeln!(
            writer,
            "{}\t{}\t{}",
            self.timestamp.format("%H:%M:%S"),
            self.url.host_str().unwrap(),
            self.url.path()
        )
    }
}

fn handler(event: S3Event) -> impl Future<Item = (), Error = failure::Error> {
    lazy_static! {
        static ref S3: S3Client = S3Client::new(Default::default());
    }

    join_all(event.records.into_iter().map(|record| {
        let bucket = record.s3.bucket.name.unwrap();
        let bucket_clone = bucket.clone();
        let key = record.s3.object.key.unwrap();
        let filename = key.rsplit('/').next().unwrap().to_owned();

        S3.get_object(GetObjectRequest {
            bucket: bucket.clone(),
            key: key.clone(),
            ..Default::default()
        })
        .from_err()
        .and_then(|response| {
            let body = BufReader::new(GzDecoder::new(response.body.unwrap().into_blocking_read()));
            let mut map = HashMap::new();
            for line in body.lines() {
                if let Some(line) = LogLine::parse(&line?) {
                    map.entry(line.key()).or_insert_with(Vec::new).push(line);
                }
            }
            Ok(map)
        })
        .and_then(move |map| {
            join_all(map.into_iter().map(move |(date, lines)| {
                let partitioned_key =
                    format!("partitioned/date={}/{}", date.format("%Y-%m-%d"), filename,);
                let mut body = GzEncoder::new(Vec::new(), Default::default());
                for line in lines {
                    line.write_to(&mut body).unwrap();
                }
                S3.put_object(PutObjectRequest {
                    bucket: bucket.clone(),
                    key: partitioned_key,
                    body: Some(body.finish().unwrap().into()),
                    ..Default::default()
                })
                .from_err()
            }))
        })
        .and_then(move |_| {
            S3.delete_object(DeleteObjectRequest {
                bucket: bucket_clone,
                key,
                ..Default::default()
            })
            .from_err()
        })
    }))
    .map(|_| ())
}

fn main() {
    lambda!(|event, _| handler(event).map_err(failure::Error::compat));
}
