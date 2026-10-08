class HomeController < ::ActionController::Base
  def create
    flash.now[:alert] = "msg"
  end
end
