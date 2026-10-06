class HomeController < ::ApplicationController
  def create
    flash.now[:alert] = "msg"
  end
end
