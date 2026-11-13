use chunked_transfer::Encoder;
use image::{self};
use servo_fetch::{FetchOptions, fetch, load_cookies};
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

const KINDLE_WIDTH: u32 = 1072;
const KINDLE_HEIGHT: u32 = 1448;

#[tokio::main]
async fn main() -> io::Result<()> {
    let token = std::fs::read_to_string("secret_token").expect("Failed to read token.");
    let bearer_token = format!("Bearer {}", token);

    let server = std::fs::read_to_string("server.conf").expect("Failed to read server.");

    let page = fetch(
        &FetchOptions::screenshot(&server, true)
            .timeout(Duration::from_secs(60))
            .settle(Duration::from_millis(3000))
            .headers(
                servo_fetch::headers::from_pairs([("Authorization", bearer_token)])
                    .expect("failed token"),
            ), //.cookies(load_cookies("cookies.txt").expect("failed token"))
    )
    .await
    .expect("Failed fetching site!");

    let image_content = page.screenshot_png().unwrap();
    std::fs::write("page.png", image_content).expect("Failed to write color image!");

    //img2.write_to(&mut Cursor::new(&mut image_content), image::ImageFormat::Png)?;
    let img = image::ImageReader::open("page.png")
        .expect("Failed to open image!")
        .decode()
        .expect("Failed to decode image!");
    let box_vertex_x = img.width().saturating_sub(KINDLE_WIDTH) / 2;
    let crop_img =
        image::imageops::crop_imm(&img, box_vertex_x, 0, KINDLE_WIDTH, KINDLE_HEIGHT).to_image();

    let gray_img = image::imageops::colorops::grayscale(&crop_img);

    gray_img.save("gray_page.png").expect("Failed saving grayscale image!");
    let host_address = "127.0.0.1:5000";

    println!("Hosting at http://{}", host_address);
    let listener = TcpListener::bind(host_address).await?;

    loop {
        let (socket, _) = listener.accept().await?;

        tokio::spawn(async move {
            // Process each socket concurrently.
            handle_connection(socket).await
        });
    }
}

async fn handle_connection(mut stream: TcpStream) {
    let frog = std::fs::read("gray_page.png").expect("blaa");

    let buf_reader = BufReader::new(&mut stream);
    let request_line = buf_reader.lines().next_line().await.unwrap().unwrap();
    if request_line == "GET / HTTP/1.1" {
        let mut encoded = Vec::new();
        {
            let mut encoder = Encoder::with_chunks_size(&mut encoded, 8);
            encoder.write_all(&frog).unwrap();
        }

        let headers = [
            "HTTP/1.1 200 OK",
            "Content-type: image/png",
            "Transfer-Encoding: chunked",
            "\r\n",
        ];
        let mut response = headers.join("\r\n").to_string().into_bytes();
        response.extend(encoded);

        match stream.write(&response).await {
            Ok(_) => println!("Response sent"),
            Err(e) => println!("Failed sending response: {}", e),
        }
    } else {
        // some other request
        println!("Unhandled request: {}", request_line);
    }
}
