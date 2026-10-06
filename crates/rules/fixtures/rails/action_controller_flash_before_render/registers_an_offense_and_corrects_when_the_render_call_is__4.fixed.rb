class HomeController < ActionController::Base
  def create
    flash.now[:alert] = "msg"
    render :index
  end
end
