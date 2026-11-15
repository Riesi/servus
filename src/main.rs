use chunked_transfer::Encoder;
use image::{self};
use servo_fetch::{Client, FetchOptions};
use std::io::{Cursor, Write};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;
use tokio::time::Instant;

mod utils;

const KINDLE_WIDTH: u32 = 1072;
const KINDLE_HEIGHT: u32 = 1448;

#[tokio::main]
async fn main() -> io::Result<()> {
    let credentials = if let Ok(cred) = utils::read_config() {
        cred
    } else {
        utils::write_example_config();
        println!("Exiting ...");
        return Ok(());
    };

    let client = Client::builder()
        .timeout(Duration::from_secs(60))
        .settle(Duration::from_millis(1000))
        .build();

    // store authentication in local storage
    let val = format!(
        "{{\"access_token\":\"{}\",\"token_type\":\"Bearer\",\"hassUrl\":\"{}\"}}",
        &credentials.token, &credentials.url
    );
    let script = format!(
        "localStorage.setItem('hassTokens', '{val}');\n
                                  JSON.stringify(localStorage);"
    );

    client
        .fetch_with(
            &FetchOptions::javascript(&credentials.url, script).settle(Duration::from_millis(200)),
        )
        .await
        .expect("Failed to populate local store!");

    let url = format!("{}{}", &credentials.url, &credentials.url_path);
    let fetch_option = FetchOptions::screenshot(&url, true, KINDLE_WIDTH, KINDLE_HEIGHT);
    let img_bytes_rw = Arc::new(RwLock::new(Vec::<u8>::new()));
    let img_bytes_w = img_bytes_rw.clone();
    tokio::spawn(async move {
        loop {
            //grab dashboard
            let page = client
                .fetch_with(&fetch_option)
                .await
                .expect("Failed fetching site!");
            let image_content = page.screenshot_png().unwrap();

            // Process page screenshot
            let img = image::ImageReader::new(Cursor::new(image_content))
                .with_guessed_format()
                .expect("Cursor io never fails")
                .decode()
                .expect("Failed to decode image!");

            let box_vertex_x = img.width().saturating_sub(KINDLE_WIDTH) / 2;
            let crop_img =
                image::imageops::crop_imm(&img, box_vertex_x, 0, KINDLE_WIDTH, KINDLE_HEIGHT)
                    .to_image();
            let gray_img = image::imageops::colorops::grayscale(&crop_img);

            // gray_img
            //     .save("gray_page.png")
            //     .expect("Failed saving grayscale image!");
            let mut img_bytes: Vec<u8> = Vec::new();
            gray_img
                .write_to(&mut Cursor::new(&mut img_bytes), image::ImageFormat::Png)
                .expect("Failed to convert image to byte stream!");

            let mut img_w = img_bytes_w.write().await;
            *img_w = img_bytes;

            tokio::time::sleep_until(Instant::now() + Duration::from_millis(2000)).await;
        }
    });

    let host_address = "127.0.0.1:5000";
    println!("Hosting at http://{}", host_address);
    let listener = TcpListener::bind(host_address).await?;
    loop {
        let (socket, _) = listener.accept().await?;
        let img_bytes_share = img_bytes_rw.clone();
        tokio::spawn(async move {
            // Process each socket concurrently.
            handle_connection(socket, &img_bytes_share).await
        });
    }
}

async fn handle_connection(mut stream: TcpStream, img_share: &Arc<RwLock<Vec<u8>>>) {
    let img_read = img_share.read().await;

    let buf_reader = BufReader::new(&mut stream);
    let request_line = buf_reader.lines().next_line().await.unwrap().unwrap();
    if request_line == "GET / HTTP/1.1" {
        let mut encoded = Vec::new();
        {
            let mut encoder = Encoder::with_chunks_size(&mut encoded, 8);
            encoder.write_all(&img_read).unwrap();
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
        //println!("Unhandled request: {}", request_line);
    }
}
