use crate::helpers::{assert_is_redirect_to, spawn_app};

#[tokio::test]
async fn an_error_flash_message_is_set_on_failure() {
    // Arrange
    let app = spawn_app().await;

    // Act
    let login_body = serde_json::json!({
      "username": "random-username",
      "password": "random-password",
    });
    let response = app.post_login(&login_body).await;

    // Assert
    assert_is_redirect_to(&response, "/login");

    // Act - Part 2
    let html_page = app.get_login_html().await;
    assert!(html_page.contains("Authentication failed"));

    // Act - Part 3
    let html_page = app.get_login_html().await;
    assert!(!html_page.contains("Authentication failed"));
}
