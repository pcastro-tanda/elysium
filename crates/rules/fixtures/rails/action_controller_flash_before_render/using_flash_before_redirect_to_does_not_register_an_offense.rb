class HomeController < ApplicationController
  def create
    flash[:alert] = "msg"
    redirect_to "https://www.example.com/"
  end
end
