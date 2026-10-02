class HomeController < ApplicationController
  def create
  rescue
    flash[:alert] = 'msg'
  ensure
    redirect_to :index
  end
end
