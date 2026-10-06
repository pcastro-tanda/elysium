class HomeController < ActionController::Base
  def create
    flash.now[:alert] = "msg" if condition
    render :index
  end
end
