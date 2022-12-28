use std::io::{self, Cursor};
use rodio::{Source, OutputStream, OutputStreamHandle};
use hyper::{Client, Body, Method, Request, StatusCode};
use clap::{Parser, ArgAction};
use urlencoding::encode;

const MIMIC3_HOST : &str = "http://tts.local:59125";
const MIMIC3_VOICE : &str = "en_US/vctk_low#p236";
const MIMIC3_NOISE : f32 = 0.33;
const MIMIC3_LENGTH : f32 = 1.2;

struct MimicConfig {
    host: String,
    voice: String,
    noise: f32,
    length: f32,
    ssml: bool
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about=None)]
struct Args {
    #[arg(short = None, long = "host", default_value_t = String::from(MIMIC3_HOST))]
    host: String,
    #[arg(short = 'v', long = "voice", default_value_t = String::from(MIMIC3_VOICE))]
    voice: String,
    #[arg(short, long, action = ArgAction::SetTrue)]
    ssml: bool,
    #[arg(short = 'r', long = "rate", default_value_t = MIMIC3_LENGTH)]
    length: f32,
    #[arg(short = 'n', long = "noise", default_value_t = MIMIC3_NOISE)]
    noise: f32
}

async fn process_line(data: &str, output: &OutputStreamHandle, config: &MimicConfig) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let client = Client::new();
    let url = format!("{}/api/tts?voice={}&noiseScale={}&lengthScale={}&ssml={}&audioTarget=client", config.host, config.voice, config.noise, config.length, config.ssml);
    let ssml_body_text = format!("<speak>{}</speak>", data);
    let req = Request::builder()
        .method(Method::POST)
        .uri(url.clone())
        .body(Body::from(match config.ssml {
            true => ssml_body_text.clone(),
            false => String::from(data)
        }))?;
    
    match client.request(req).await {
        Ok(response) => {
            if response.status()==StatusCode::OK {
                match hyper::body::to_bytes(response.into_body()).await {
                    Ok(buff) => {
                        let cursor = Cursor::new(buff);
                        let source = rodio::Decoder::new_wav(cursor).unwrap();
                        let duration = source.total_duration().expect("Unable to determine length of audio sample.");
                        match output.play_raw(source.convert_samples()) {
                            Ok(()) => std::thread::sleep(duration),
                            Err(error) => {
                                println!("Error playing audio clip: {}", error);
                            }
                        }
                        Ok(())
                    },
                    Err(error) => Err(Box::new(error))
                }
            } else {
                eprintln!("API Unable to process line: {}", match config.ssml {
                    true => ssml_body_text,
                    false => String::from(data)
                });
                eprintln!("URL used: {}", url);
                eprintln!("Response: {}", response.status());
                Ok(())
            }
        },
        Err(error) => {
            Err(Box::new(error))
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cli = Args::parse();

    let config : MimicConfig = MimicConfig {
        host: cli.host,
        voice: encode(&cli.voice).into_owned(),
        noise: cli.noise,
        length: cli.length,
        ssml: cli.ssml
    };

    // let matches = App::new("Mimic3 Keyboard")
    //     .version("0.1.0")
    //     .author("Lauren Kaviak <lauren@laurayco.me>")
    //     .about("Sends stdin to a mimic3 API instance and plays the result on the default audio device")
    //     .arg(Arg::new("host").short("h").long("host").takes_value(true).help("Mimic3 hostname (including protocol, eg: http://mimic3.com)"))
    //     .arg(Arg::new("voice").short("v").long("voice").takes_value(true).help("Mimic3 voice to use"))
    //     .arg(Arg::new("ssml").short("s").long("ssml").takes_value(false).help("enable ssml support"))
    //     .get_matches();

    let stdin = io::stdin();
    let (_stream, stream_handle) = OutputStream::try_default().unwrap();
    loop {
        let mut user_input = String::new();
        stdin.read_line(&mut user_input)?;
        let user_input = user_input.trim();

        if user_input.to_lowercase()=="goodbye" {
            break;
        }

        match process_line(user_input, &stream_handle, &config).await {
            Ok(..) => {},
            Err(error) => {
                println!("Error: {}", error);
            }
        }
    }
    Ok(())
}
