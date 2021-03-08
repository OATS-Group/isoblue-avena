use std::convert::TryInto;
use std::path::Path;

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, DurationRound, NaiveDateTime, TimeZone, Utc};
use geo_types::Point;
use gpx::{write, Gpx, GpxVersion, Track, TrackSegment, Waypoint};
use sqlx::{postgres::PgPoolOptions, Row};

use clap::{crate_authors, crate_description, crate_name, crate_version, App, Arg, ArgGroup};

use walkdir::WalkDir;

use ansi_term::ANSIStrings;
use ansi_term::Colour::{Blue, Green, Red, White, Yellow};

#[derive(sqlx::FromRow)]
struct GPS {
    time: DateTime<Utc>,
    lat: f64,
    lng: f64,
}

// TOOD: remove --hourly and --daily and make -l take <num|hourly|daily> so that
// simple ENV variables can be used to configure program

#[tokio::main]
async fn main() -> Result<()> {
    let matches = App::new(crate_name!())
        .version(crate_version!())
        .author(crate_authors!())
        .about(crate_description!())
        .arg(
            Arg::with_name("from")
                .short("f")
                .long("from")
                .takes_value(true)
                .validator(is_date)
                .help("Time to start querying data from"),
        )
        .arg(
            Arg::with_name("to")
                .short("t")
                .long("to")
                .takes_value(true)
                .validator(is_date)
                .help("Time to stop querying data from"),
        )
        .arg(
            Arg::with_name("truncate")
                .long("truncate")
                .takes_value(false)
                .help("Truncate the --from time by the segment length for predictable file naming"),
        )
        .arg(
            Arg::with_name("segment length")
                .short("l")
                .takes_value(true)
                .validator(is_u64)
                .help("Number of seconds per file"),
        )
        .arg(
            Arg::with_name("daily")
                .long("daily")
                .takes_value(false)
                .help("Use a daily segment size. Implies -l 86400 and --truncate"),
        )
        .arg(
            Arg::with_name("hourly")
                .long("hourly")
                .takes_value(false)
                .help("Use a hourly segment size. Implies -l 3600 and --truncate"),
        )
        .group(
            ArgGroup::with_name("segment")
                .args(&["segment length", "daily", "hourly"])
                .multiple(false)
                .required(true),
        )
        .arg(
            Arg::with_name("append")
                .long("append")
                .takes_value(false)
                .help(
                    "Only append to existing exported collection in out_dir.
                    The from time is fast forwarded to equal the start time of
                    the most recent data file in the output directory. Files
                    with older data will not be modified.",
                ),
        )
        .arg(
            Arg::with_name("database")
                .index(1)
                .required(true)
                .help("Avena database connection string"),
        )
        .arg(
            Arg::with_name("out_dir")
                .index(2)
                .required(true)
                .help("Path to write rendered files to"),
        )
        .get_matches();

    let conninfo = matches
        .value_of("database")
        .with_context(|| "Database connection info not provided")?;

    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(conninfo)
        .await
        .with_context(|| format!("Could not connect to {}", conninfo))?;

    // Use provided from time, otherwise default to first time in database
    let mut from = match matches.value_of("from") {
        Some(date) => parse_date(date)?,
        None => sqlx::query("SELECT time FROM gps LIMIT 1")
            .fetch_one(&pool)
            .await?
            .get("time"),
    };

    // Use provided to time, otherwise default to first time in database
    let to = match matches.value_of("to") {
        Some(date) => parse_date(date)?,
        None => sqlx::query("SELECT time FROM gps ORDER BY time DESC LIMIT 1")
            .fetch_one(&pool)
            .await?
            .get(0),
    };

    let mut segment_length: u64 = match matches.value_of("segment length") {
        Some(length) => length.parse()?,
        None => (to.timestamp() - from.timestamp()).try_into()?,
    };

    let mut truncate = matches.is_present("truncate");

    if matches.is_present("hourly") {
        segment_length = 3600;
        truncate = true;
    } else if matches.is_present("daily") {
        segment_length = 86400;
        truncate = true;
    }

    if truncate {
        from = from.duration_trunc(Duration::seconds(segment_length as i64))?;
    }

    let out_dir = Path::new(
        matches
            .value_of("out_dir")
            .with_context(|| "Output directory is required.")?,
    );

    if !out_dir.exists() {
        std::fs::create_dir(out_dir).with_context(|| "Can not create output directory")?;
    }

    // TODO: --append implantation is a bit ... nasty?
    let mut max_date = from;
    if matches.is_present("append") {
        let gpx_files = WalkDir::new(out_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .map(|e| e.path().to_owned())
            .filter(|p| p.extension().unwrap_or_default() == "gpx");

        for file in gpx_files {
            let ts_str = file.file_stem().unwrap_or_default().to_string_lossy();

            if let Ok(ts) = DateTime::parse_from_rfc3339(&ts_str) {
                if ts > max_date {
                    max_date = ts.with_timezone(&Utc);
                }
            }
        }
    }

    if max_date != from {
        println!(
            "{}\n",
            ANSIStrings(&[
                Yellow.italic().paint("Note: "),
                Yellow.paint("Updating from time to "),
                Yellow.bold().paint(max_date.to_rfc2822()),
                Yellow.paint(" based on existing data.")
            ])
        );
        from = max_date;
    }

    println!("🎉 {}", Green.bold().paint("Exporting GPS"));
    println!("From: {}", White.bold().paint(from.to_rfc2822()));
    println!("To: {}", White.bold().paint(to.to_rfc2822()));
    println!(
        "File span: {} seconds\n",
        White.bold().paint(segment_length.to_string())
    );

    for t in (from.timestamp()..to.timestamp()).step_by(segment_length as usize) {
        let start = Utc.from_utc_datetime(&NaiveDateTime::from_timestamp(t, 0));
        let stop = start + Duration::seconds(segment_length as i64);
        let filename = out_dir
            .join(Path::new(&start.to_rfc3339()))
            .with_extension("gpx");

        print!(
            "{} ",
            ANSIStrings(&[
                Blue.paint("Writing segment: "),
                Blue.bold().paint(
                    filename
                        .file_name()
                        .unwrap_or_default()
                        .to_str()
                        .unwrap_or_default()
                )
            ])
        );

        let recs: Vec<GPS> = sqlx::query_as(
            r#"
            SELECT time, lat, lng
            FROM gps
            WHERE time BETWEEN $1 AND $2
            ORDER BY time
            "#,
        )
        .bind(&start)
        .bind(&stop)
        .fetch_all(&pool)
        .await?;

        if recs.len() == 0 {
            println!("{}", Yellow.paint("⇝ No data"));
            continue;
        }

        let mut segment = TrackSegment::new();
        for rec in recs {
            let mut wpt = Waypoint::new(Point::new(rec.lng, rec.lat));
            wpt.time = Some(rec.time);

            segment.points.push(wpt);
        }

        let mut track = Track::new();
        track.segments.push(segment);

        let mut data: Gpx = Default::default();
        data.version = GpxVersion::Gpx11;
        data.tracks.push(track);

        let path = Path::new(out_dir).join(Path::new(&start.to_rfc3339()).with_extension("gpx"));
        let out_file = std::fs::File::create(&path)?;

        match write(&data, out_file) {
            Ok(_) => println!("{}", Green.paint("✓")),
            Err(_) => eprintln!("{} {:?}", Red.bold().paint("⨯ "), path),
        };
    }

    println!(
        "\n{}\n",
        Green.bold().paint("🥳 Export complete. Enjoy your 📊!")
    );

    Ok(())
}

fn parse_date(date: &str) -> anyhow::Result<DateTime<Utc>> {
    Utc.datetime_from_str(date, "%Y-%m-%d %H:%M:%S")
        .with_context(|| "Use YYYY-MM-DD HH:MM:SS")
}

fn is_date(date: String) -> Result<(), String> {
    match parse_date(&date) {
        Ok(_) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

fn is_u64(num: String) -> Result<(), String> {
    match num.parse::<u64>() {
        Ok(_) => Ok(()),
        Err(_) => Err(format!("Must be a positive integer")),
    }
}

/*
#[derive(Debug)]
struct MyRecord {
    time: String,
    test: f64,
}

impl shapefile::dbase::WritableRecord for MyRecord {
    fn write_using<'a, W: std::io::Write>(
        &self,
        field_writer: &mut shapefile::dbase::FieldWriter<'a, W>,
    ) -> Result<(), shapefile::dbase::FieldIOError> {
        field_writer.write_next_field_value(&self.time)?;
        field_writer.write_next_field_value(&self.test)?;
        Ok(())
    }
}

fn to_shape(recs: Vec<GPS>) -> (Vec<shapefile::record::Point>, Vec<MyRecord>) {
    recs.into_iter()
        .map(|rec| {
            let points = shapefile::record::Point::new(rec.lng, rec.lat);
            let h = MyRecord {
                time: rec.time.to_rfc3339().to_owned(),
                test: 12.3,
            };
            (points, h)
        })
        .unzip()
}

        let (points, records) = to_shape(recs);

        let table_builder = shapefile::dbase::TableWriterBuilder::new()
            .add_character_field("time".try_into().unwrap(), 50)
            .add_numeric_field("TestFlat".try_into().unwrap(), 6, 2);

        let writer = shapefile::Writer::from_path(
            format!("{}/{}.shp", out_dir, f.to_rfc3339()),
            table_builder,
        )?;
        writer.write_shapes_and_records(&points[0..2], &records[0..2])?;
        */
