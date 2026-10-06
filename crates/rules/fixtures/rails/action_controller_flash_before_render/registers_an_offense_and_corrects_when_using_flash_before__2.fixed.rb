class HomeController < ApplicationController
  def create
    flash.now[:alert] = "msg" if condition
    render :index
  end
end
