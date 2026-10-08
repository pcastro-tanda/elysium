class HomeController < ActionController::Base
  before_action do
    flash.now[:alert] = "msg"
    render :index
  end
end
