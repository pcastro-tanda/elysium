class HomeController < ApplicationController
  before_action do
    flash[:alert] = "msg"
    ^^^^^ Use `flash.now` before `render`.
    render :index
  end
end
