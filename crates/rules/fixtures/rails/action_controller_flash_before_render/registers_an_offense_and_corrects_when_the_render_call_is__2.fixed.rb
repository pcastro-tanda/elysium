class HomeController < ::ApplicationController
  def create
    flash.now[:alert] = "msg"
    render :index
  end
end
