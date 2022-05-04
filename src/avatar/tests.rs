use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path};
use crate::avatar::get_url_data;
use crate::model::Url;

#[tokio::test]
async fn test_get_url_data() {
    // Start a background HTTP server on a random local port
    let mock_server = MockServer::start().await;
    let data = "1234".as_bytes();
    let template =
      ResponseTemplate::new(200)
        .set_body_bytes(data);

    Mock::given(method("GET"))
        .and(path("/u/12345"))
        .respond_with(template)
        // Mounting the mock on the mock server - it's now effective!
        .mount(&mock_server)
        .await;

    let url = Url::new(format!("{}/u/12345?v=4", &mock_server.uri()));
    let result = get_url_data(url).await.unwrap();

    assert_eq!(result.1, data)
}
