include Rack::Test::Methods

get :create, user_id: @user.id
