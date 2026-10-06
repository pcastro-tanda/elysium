class HomeController < ApplicationController
  def create
    if condition
      flash[:alert] = "msg"
    end

    redirect_back fallback_location: root_path
  end
end
