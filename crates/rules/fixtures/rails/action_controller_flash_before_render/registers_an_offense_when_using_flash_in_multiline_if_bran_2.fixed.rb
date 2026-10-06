class HomeController < ApplicationController
  def create
    if condition
      do_something
      flash.now[:alert] = "msg"
    end

    render :index
  end
end
