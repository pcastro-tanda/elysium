class HomeController < ApplicationController
  def create
    flash[:alert] = "msg" if condition
    ^^^^^ Use `flash.now` before `render`.
    render :index
  end
end
