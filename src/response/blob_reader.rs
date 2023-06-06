use axum::body;
use axum::response::{IntoResponse, Response};
use tokio_util::codec::{BytesCodec, FramedRead};

use crate::registry_interface::BlobReader;

impl IntoResponse for BlobReader {
    fn into_response(self) -> Response {
        let digest = self.digest().to_string();
        let stream = FramedRead::new(self.get_reader(), BytesCodec::new());

        Response::builder()
            .header("Content-Type", "application/octet-stream")
            .header("Docker-Content-Digest", digest)
            .body(body::StreamBody::new(stream))
            .unwrap()
            .into_response()
    }
}
