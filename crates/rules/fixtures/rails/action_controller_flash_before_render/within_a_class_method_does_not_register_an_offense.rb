class HomeController < ApplicationController
  def self.create
    flash[:alert] = "msg"
    render :index
  end
end
