use control::repo::connector_images::fetch_all;

use crate::support::redactor::Redactor;
use crate::support::{self, factory, test_context};

#[tokio::test]
async fn index_test() {
    // Arrange
    let t = test_context!();
    let connector = factory::HelloWorldConnector.create(t.db()).await;
    let image = factory::HelloWorldImage.create(t.db(), &connector).await;

    // Act
    let response = t.get("/connector_images").await;

    // Assert
    assert!(response.status().is_success());
    let redactor = Redactor::default()
        .redact(connector.id, "c1")
        .redact(image.id, "i1");
    assert_json_snapshot!(redactor.response_json(response).await.unwrap(), {
        ".data.*.attributes.created_at" => "[datetime]",
        ".data.*.attributes.updated_at" => "[datetime]",
    });
}

#[tokio::test]
async fn create_test() {
    // Arrange
    let t = test_context!();
    let connector = factory::HelloWorldConnector.create(t.db()).await;
    let input = factory::HelloWorldImage.attrs(&connector);

    // Act
    let response = t.post("/connector_images", &input).await;

    // Assert
    let images = fetch_all(t.db()).await.expect("to insert test data");
    assert_eq!(1, images.len());
    assert_eq!(201, response.status().as_u16());
    let redactor = Redactor::default()
        .redact(connector.id, "c1")
        .redact(images[0].id, "i1");
    assert_json_snapshot!(redactor.response_json(response).await.unwrap(), {
        ".data.attributes.created_at" => "[datetime]",
        ".data.attributes.updated_at" => "[datetime]",
    });
}

#[tokio::test]
async fn show_test() {
    // Arrange
    let t = test_context!();
    let connector = factory::HelloWorldConnector.create(t.db()).await;
    let image = factory::HelloWorldImage.create(t.db(), &connector).await;

    // Act
    let response = t.get(&format!("/connector_images/{}", &image.id)).await;

    // Assert
    assert_eq!(200, response.status().as_u16());
    let redactor = Redactor::default()
        .redact(connector.id, "c1")
        .redact(image.id, "i1");
    assert_json_snapshot!(redactor.response_json(response).await.unwrap(), {
        ".data.attributes.created_at" => "[datetime]",
        ".data.attributes.updated_at" => "[datetime]",
    });
}

#[tokio::test]
async fn spec_test() {
    // Arrange
    let t = test_context!();
    let connector = factory::HelloWorldConnector.create(t.db()).await;
    let image = factory::HelloWorldImage.create(t.db(), &connector).await;

    // Act
    let response = t
        .get(&format!("/connector_images/{}/spec", &image.id))
        .await;

    // Assert
    assert_eq!(200, response.status().as_u16());
    let redactor = Redactor::default()
        .redact(connector.id, "c1")
        .redact(image.id, "i1");
    assert_json_snapshot!(redactor.response_json(response).await.unwrap(), {
        ".data.id" => "[nonce]",
        ".data.attributes.created_at" => "[datetime]",
        ".data.attributes.updated_at" => "[datetime]",
    });
}

#[tokio::test]
async fn discovery_test() {
    // Arrange
    let t = test_context!();
    let connector = factory::HelloWorldConnector.create(t.db()).await;
    let image = factory::HelloWorldImage.create(t.db(), &connector).await;
    let config = serde_json::json!({"greetings": 10});

    // Act
    let response = t
        .post(
            &format!("/connector_images/{}/discovery", &image.id),
            &config,
        )
        .await;

    // Assert
    assert_eq!(200, response.status().as_u16());
    let redactor = Redactor::default()
        .redact(connector.id, "c1")
        .redact(image.id, "i1");
    assert_json_snapshot!(redactor.response_json(response).await.unwrap(), {
        ".data.id" => "[nonce]",
        ".data.attributes.created_at" => "[datetime]",
        ".data.attributes.updated_at" => "[datetime]",
    });
}
