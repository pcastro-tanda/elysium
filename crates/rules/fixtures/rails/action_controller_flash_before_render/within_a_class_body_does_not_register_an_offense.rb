class HomeController < ApplicationController
  flash[:alert] = "msg"
  render :index
end
