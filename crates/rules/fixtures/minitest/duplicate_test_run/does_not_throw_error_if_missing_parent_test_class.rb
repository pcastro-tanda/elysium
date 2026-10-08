class PostmarkAccountServiceTest < ActionDispatch::IntegrationTest
  test "it handles missing payloads from Postmark Account API errors" do
    Postmark::AccountApiClient.any_instance.stubs(:create_server).raises(StandardError)

    Rails.logger.expects(:error)
    PostmarkAccountService.create_server({})
  end
end
