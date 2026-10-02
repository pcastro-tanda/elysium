class HomeController < ::ApplicationController
  def create
    flash[:alert] = "msg"
    ^^^^^ Use `flash.now` before `render`.
    render :index
  end
end
