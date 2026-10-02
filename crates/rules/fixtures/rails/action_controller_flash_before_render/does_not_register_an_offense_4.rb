class HomeController < ApplicationController
  def create
    render :index and return if foo?
    flash[:alert] = "msg"
    redirect_to "https://www.example.com/"
  end
end
